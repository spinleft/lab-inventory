pub use crate::test_user::TestUser;
use lab_inventory::configuration::{DatabaseSettings, get_configuration};
use lab_inventory::startup::{Application, get_connection_pool};
use lab_inventory::telemetry::{get_subscriber, init_subscriber};
use secrecy::Secret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::LazyLock;
use uuid::Uuid;

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

/// Drives the reclamation of finished tests' databases.
///
/// [`TestApp`] cannot close its pools from `Drop`, which is synchronous, and a
/// task spawned onto the test's own runtime would never run: `#[tokio::test]`
/// drops that runtime as soon as the test body returns, which is exactly why the
/// connections leaked in the first place. This runtime is a process-wide static,
/// so whatever is handed to it always runs to completion.
static CLEANUP: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Failed to build the test cleanup runtime.")
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
    pub local_node_id: Option<Uuid>,
    database_settings: DatabaseSettings,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let maintenance = DatabaseSettings {
            database_name: "postgres".to_string(),
            username: "postgres".to_string(),
            password: Secret::new("password".to_string()),
            ..self.database_settings.clone()
        };
        let database_name = self.database_settings.database_name.clone();
        CLEANUP.spawn(async move {
            let Ok(mut connection) =
                PgConnection::connect_with(&maintenance.connect_options()).await
            else {
                return;
            };
            // FORCE terminates whatever the finished test left connected. The
            // application's own pool is the reason that is needed: it lives on
            // actix worker threads that outlive the test runtime, so nothing on
            // this side can ever close it.
            let _ = connection
                .execute(
                    format!(r#"DROP DATABASE IF EXISTS "{database_name}" WITH (FORCE);"#).as_str(),
                )
                .await;
            let _ = connection.close().await;
        });
    }
}

pub async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.application.cookie_secure = false;
        c.file_storage.local_root = std::env::temp_dir()
            .join(format!("lab-inventory-test-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");
    let application_port = application.port();
    std::mem::drop(tokio::spawn(application.run_until_stopped()));

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        address: format!("http://localhost:{application_port}"),
        db_pool: get_connection_pool(&configuration.database),
        test_user: TestUser::generate_with_user_type("super_admin", None),
        api_client: client,
        local_node_id: Some(Uuid::new_v4()),
        database_settings: configuration.database.clone(),
    };

    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

/// Runs the pairing handshake between two instances and returns the remote node
/// id `local` now knows `remote` by.
///
/// Leaves both apps logged in as a freshly created `lab_admin` of their own
/// laboratory, which is what the handshake needs; callers that want a different
/// session log in again afterwards.
pub async fn pair_laboratories(
    local: &TestApp,
    local_laboratory_id: Uuid,
    remote: &TestApp,
    remote_laboratory_id: Uuid,
) -> Uuid {
    let remote_admin = TestUser::generate_with_user_type("lab_admin", Some(remote_laboratory_id));
    remote.store_user(&remote_admin).await;
    remote_admin.login(remote).await;
    let response = remote
        .post_federation_pairing_code(remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let pairing: serde_json::Value = response.json().await.unwrap();
    let pairing_code = pairing["pairing_code"].as_str().unwrap();

    let local_admin = TestUser::generate_with_user_type("lab_admin", Some(local_laboratory_id));
    local.store_user(&local_admin).await;
    local_admin.login(local).await;
    let response = local
        .post_federation_trust(
            local_laboratory_id,
            &serde_json::json!({
                "remote_base_url": remote.address,
                "remote_laboratory_id": remote_laboratory_id,
                "pairing_code": pairing_code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let trust: serde_json::Value = response.json().await.unwrap();
    let response = local.get_federation_trusts(local_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let trusts: serde_json::Value = response.json().await.unwrap();
    assert_eq!(trusts.as_array().unwrap().len(), 1);
    trust["remote_node_id"].as_str().unwrap().parse().unwrap()
}

/// The headers a partner would send, built from the shared secret the handshake
/// stored, so a test can put a signed request on the wire without going through
/// the proxy.
///
/// It reproduces `sign_canonical` rather than calling it — that function is
/// private to the crate's federation module. Signing here over a *different*
/// body than the one sent is how the body-tampering test is written.
pub async fn sign_federation_request(
    sender: &TestApp,
    receiver: &TestApp,
    method: &str,
    path_and_query: &str,
    signed_body: &[u8],
    caller_laboratory_id: Uuid,
    caller_user_id: Uuid,
    caller_user_type: &str,
) -> Vec<(String, String)> {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};

    // The node id on the wire is the sender's own identity, which the receiver
    // recorded as one of its known remote nodes during the handshake.
    let sender_node_id = sqlx::query_scalar!("SELECT node_id FROM federation_local_nodes")
        .fetch_one(&sender.db_pool)
        .await
        .expect("Failed to read the sender's federation node identity.");
    let record = sqlx::query!(
        "SELECT shared_secret, key_version FROM federation_remote_nodes WHERE remote_node_id = $1",
        sender_node_id
    )
    .fetch_one(&receiver.db_pool)
    .await
    .expect("Failed to read the federation shared secret.");

    let body_hash: String = Sha256::digest(signed_body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = Uuid::new_v4().to_string();
    let canonical = format!(
        "v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        method.to_uppercase(),
        path_and_query,
        body_hash,
        sender_node_id,
        caller_laboratory_id,
        caller_user_id,
        caller_user_type,
        timestamp,
        nonce,
        record.key_version,
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(record.shared_secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(canonical.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());

    vec![
        ("x-federation-node-id".into(), sender_node_id.to_string()),
        (
            "x-federation-key-version".into(),
            record.key_version.to_string(),
        ),
        ("x-federation-timestamp".into(), timestamp),
        ("x-federation-nonce".into(), nonce),
        ("x-federation-signature".into(), signature),
        (
            "x-federation-remote-laboratory-id".into(),
            caller_laboratory_id.to_string(),
        ),
        (
            "x-federation-remote-user-id".into(),
            caller_user_id.to_string(),
        ),
        ("x-federation-remote-username".into(), "tamper".into()),
        (
            "x-federation-remote-user-type".into(),
            caller_user_type.into(),
        ),
    ]
}

impl TestApp {
    pub async fn get_health_check(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/health_check", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/auth/login", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(format!("{}/api/v1/auth/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_auth_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/auth/password", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_guest_registration_code(&self, _laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .post(format!(
                "{}/api/v1/local/guest-registration-codes",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_guest_registration<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/auth/guest-registration", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_me(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/auth/me", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_api_path(&self, path_and_query: &str) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1{path_and_query}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    async fn current_session_is_system_admin(&self) -> bool {
        let response = self
            .api_client
            .get(format!("{}/api/v1/auth/me", &self.address))
            .send()
            .await
            .expect("Failed to inspect the current test session.");
        if !response.status().is_success() {
            return false;
        }
        let body: serde_json::Value = response
            .json()
            .await
            .expect("Failed to deserialize the current test user.");
        matches!(
            body.pointer("/user_type/name")
                .and_then(|value| value.as_str()),
            Some("root" | "super_admin")
        )
    }

    async fn laboratory_api_path(&self, laboratory_id: Uuid, tail: &str) -> String {
        let tail = tail.trim_matches('/');
        if self.current_session_is_system_admin().await {
            if tail.is_empty() {
                format!("/api/v1/admin/laboratories/{laboratory_id}")
            } else {
                format!("/api/v1/admin/laboratories/{laboratory_id}/{tail}")
            }
        } else if tail.is_empty() {
            "/api/v1/local/laboratory".to_string()
        } else {
            format!("/api/v1/local/{tail}")
        }
    }

    async fn resource_api_path(
        &self,
        table: &str,
        id_column: &str,
        resource_id: Uuid,
        tail: &str,
    ) -> String {
        if !self.current_session_is_system_admin().await {
            return format!("/api/v1/local/{}", tail.trim_matches('/'));
        }
        let query = format!("SELECT laboratory_id FROM {table} WHERE {id_column} = $1");
        let laboratory_id = sqlx::query_scalar::<_, Uuid>(&query)
            .bind(resource_id)
            .fetch_optional(&self.db_pool)
            .await
            .expect("Failed to find the resource laboratory for a test request.")
            .or(sqlx::query_scalar::<_, Uuid>(
                "SELECT laboratory_id FROM laboratories ORDER BY created_at LIMIT 1",
            )
            .fetch_optional(&self.db_pool)
            .await
            .expect("Failed to find a fallback laboratory for a test request."))
            .unwrap_or_else(Uuid::nil);
        self.laboratory_api_path(laboratory_id, tail).await
    }

    async fn users_api_path(&self, tail: &str) -> String {
        let prefix = if self.current_session_is_system_admin().await {
            "/api/v1/admin/users"
        } else {
            "/api/v1/local/users"
        };
        let tail = tail.trim_matches('/');
        if tail.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}/{tail}")
        }
    }

    pub async fn post_laboratory<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/admin/laboratories", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratories(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/admin/laboratories", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratory(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self.laboratory_api_path(laboratory_id, "").await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_laboratory<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.laboratory_api_path(laboratory_id, "").await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_laboratory(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/admin/laboratories/{laboratory_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_federation_pairing_code(&self, _laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .post(format!(
                "{}/api/v1/local/federation/pairing-codes",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_federation_trust<Body>(
        &self,
        _laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/local/federation/trusts", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_federation_trusts(&self, _laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/local/federation/trusts", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_federation_guest_links(&self, _laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/local/federation/guest-links",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn merge_federation_guest_link<Body>(
        &self,
        _laboratory_id: Uuid,
        link_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/local/federation/guest-links/{link_id}/merge",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_federation_assets(
        &self,
        remote_node_id: Uuid,
        remote_laboratory_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/assets",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_federation_attachment(
        &self,
        remote_node_id: Uuid,
        remote_laboratory_id: Uuid,
        attachment_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/attachments/{attachment_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset_category<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .laboratory_api_path(laboratory_id, "asset-categories")
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_categories(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self
            .laboratory_api_path(laboratory_id, "asset-categories")
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_categories_under(
        &self,
        laboratory_id: Uuid,
        root_category_id: Uuid,
    ) -> reqwest::Response {
        let path = self
            .laboratory_api_path(
                laboratory_id,
                &format!("asset-categories?root_category_id={root_category_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_category(&self, category_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_categories",
                "category_id",
                category_id,
                &format!("asset-categories/{category_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_asset_category<Body>(
        &self,
        category_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_categories",
                "category_id",
                category_id,
                &format!("asset-categories/{category_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset_category(&self, category_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_categories",
                "category_id",
                category_id,
                &format!("asset-categories/{category_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset_parameter<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .laboratory_api_path(laboratory_id, "asset-parameters")
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_parameters(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self
            .laboratory_api_path(laboratory_id, "asset-parameters")
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_parameter(&self, parameter_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_parameter_types",
                "parameter_type_id",
                parameter_id,
                &format!("asset-parameters/{parameter_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_asset_parameter<Body>(
        &self,
        parameter_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_parameter_types",
                "parameter_type_id",
                parameter_id,
                &format!("asset-parameters/{parameter_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset_parameter(&self, parameter_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_parameter_types",
                "parameter_type_id",
                parameter_id,
                &format!("asset-parameters/{parameter_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset<Body>(&self, laboratory_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.laboratory_api_path(laboratory_id, "assets").await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn upload_file(
        &self,
        laboratory_id: Uuid,
        file_name: &str,
        mime_type: &str,
        bytes: Vec<u8>,
    ) -> reqwest::Response {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .expect("Invalid file MIME type");
        let form = reqwest::multipart::Form::new().part("file", part);
        let path = self
            .laboratory_api_path(laboratory_id, "file-uploads")
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .multipart(form)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_file_upload(&self, upload_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "file_uploads",
                "upload_id",
                upload_id,
                &format!("file-uploads/{upload_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset_attachment<Body>(
        &self,
        asset_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}/attachments"),
            )
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_inventory_item_attachment<Body>(
        &self,
        inventory_item_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}/attachments"),
            )
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_attachments(&self, asset_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}/attachments"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_item_attachments(
        &self,
        inventory_item_id: Uuid,
    ) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}/attachments"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratory_attachments(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self.laboratory_api_path(laboratory_id, "attachments").await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_attachment_assignments",
                "attachment_id",
                attachment_id,
                &format!("attachments/{attachment_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_attachment<Body>(
        &self,
        attachment_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_attachment_assignments",
                "attachment_id",
                attachment_id,
                &format!("attachments/{attachment_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_attachment_assignments",
                "attachment_id",
                attachment_id,
                &format!("attachments/{attachment_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn download_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_attachment_assignments",
                "attachment_id",
                attachment_id,
                &format!("attachments/{attachment_id}/download"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_assets(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self.laboratory_api_path(laboratory_id, "assets").await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_assets_with_query(
        &self,
        laboratory_id: Uuid,
        query: &str,
    ) -> reqwest::Response {
        let path = self
            .laboratory_api_path(laboratory_id, &format!("assets?{query}"))
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset(&self, asset_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_with_query(&self, asset_id: Uuid, query: &str) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}?{query}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_asset<Body>(&self, asset_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset(&self, asset_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_inventory_items<Body>(&self, asset_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "assets",
                "asset_id",
                asset_id,
                &format!("assets/{asset_id}/inventory-items"),
            )
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_items(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self
            .laboratory_api_path(laboratory_id, "inventory-items")
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_items_with_query(
        &self,
        laboratory_id: Uuid,
        query: &str,
    ) -> reqwest::Response {
        let path = self
            .laboratory_api_path(laboratory_id, &format!("inventory-items?{query}"))
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_item(&self, inventory_item_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_borrow_request<Body>(
        &self,
        inventory_item_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/local/inventory-items/{inventory_item_id}/borrow-requests",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_borrow_requests(&self, _laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/local/borrow-requests", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_my_borrow_requests(&self) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/local/borrow-requests/mine",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_borrow_request_cancel(&self, borrow_request_id: Uuid) -> reqwest::Response {
        self.api_client
            .post(format!(
                "{}/api/v1/local/borrow-requests/{borrow_request_id}/cancel",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_federation_borrow_request<Body>(
        &self,
        remote_node_id: Uuid,
        remote_laboratory_id: Uuid,
        inventory_item_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/inventory-items/{inventory_item_id}/borrow-requests",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_federation_borrow_requests(
        &self,
        remote_node_id: Uuid,
        remote_laboratory_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/borrow-requests",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_federation_borrow_request_cancel(
        &self,
        remote_node_id: Uuid,
        remote_laboratory_id: Uuid,
        borrow_request_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .post(format!(
                "{}/api/v1/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/borrow-requests/{borrow_request_id}/cancel",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_borrow_request<Body>(
        &self,
        _laboratory_id: Uuid,
        borrow_request_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!(
                "{}/api/v1/local/borrow-requests/{borrow_request_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_inventory_item<Body>(
        &self,
        inventory_item_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_inventory_items_batch<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .laboratory_api_path(laboratory_id, "inventory-items/batch")
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn split_inventory_item<Body>(
        &self,
        inventory_item_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}/split"),
            )
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn merge_inventory_items<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .laboratory_api_path(laboratory_id, "inventory-items/merge")
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_inventory_item(&self, inventory_item_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "asset_inventory_items",
                "inventory_item_id",
                inventory_item_id,
                &format!("inventory-items/{inventory_item_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn batch_delete_inventory_items<Body>(
        &self,
        laboratory_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .laboratory_api_path(laboratory_id, "inventory-items/batch-delete")
            .await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_unit<Body>(&self, laboratory_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.laboratory_api_path(laboratory_id, "units").await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_units(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self.laboratory_api_path(laboratory_id, "units").await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_unit(&self, unit_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path("units", "unit_id", unit_id, &format!("units/{unit_id}"))
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_unit<Body>(&self, unit_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path("units", "unit_id", unit_id, &format!("units/{unit_id}"))
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_unit(&self, unit_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path("units", "unit_id", unit_id, &format!("units/{unit_id}"))
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_location<Body>(&self, laboratory_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.laboratory_api_path(laboratory_id, "locations").await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_locations(&self, laboratory_id: Uuid) -> reqwest::Response {
        let path = self.laboratory_api_path(laboratory_id, "locations").await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_locations_under(
        &self,
        laboratory_id: Uuid,
        root_location_id: Uuid,
    ) -> reqwest::Response {
        let path = self
            .laboratory_api_path(
                laboratory_id,
                &format!("locations?root_location_id={root_location_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_location(&self, location_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "locations",
                "location_id",
                location_id,
                &format!("locations/{location_id}"),
            )
            .await;
        self.api_client
            .get(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_location<Body>(&self, location_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self
            .resource_api_path(
                "locations",
                "location_id",
                location_id,
                &format!("locations/{location_id}"),
            )
            .await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_location(&self, location_id: Uuid) -> reqwest::Response {
        let path = self
            .resource_api_path(
                "locations",
                "location_id",
                location_id,
                &format!("locations/{location_id}"),
            )
            .await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_user<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.users_api_path("").await;
        self.api_client
            .post(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_user<Body>(&self, user_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        let path = self.users_api_path(&user_id.to_string()).await;
        self.api_client
            .patch(format!("{}{}", &self.address, path))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_user(&self, user_id: Uuid) -> reqwest::Response {
        let path = self.users_api_path(&user_id.to_string()).await;
        self.api_client
            .delete(format!("{}{}", &self.address, path))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn store_user(&self, user: &TestUser) {
        user.store(&self.db_pool).await;
    }

    pub async fn create_laboratory(&self, name: &str) -> Uuid {
        sqlx::query_scalar(
            r#"
            INSERT INTO laboratories (laboratory_id, name, address)
            VALUES ($1, $2, $3)
            RETURNING laboratory_id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .bind(format!("{name} address"))
        .fetch_one(&self.db_pool)
        .await
        .expect("Failed to create test laboratory.")
    }

    pub async fn unit_id(&self, code: &str) -> Uuid {
        sqlx::query_scalar("SELECT unit_id FROM units WHERE code = $1")
            .bind(code)
            .fetch_one(&self.db_pool)
            .await
            .expect("Failed to fetch unit id.")
    }
}

async fn configure_database(config: &DatabaseSettings) {
    let maintenance_settings = DatabaseSettings {
        database_name: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Secret::new("password".to_string()),
        ..config.clone()
    };
    let mut connection = PgConnection::connect_with(&maintenance_settings.connect_options())
        .await
        .expect("Failed to connect to Postgres.");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");
    connection
        .close()
        .await
        .expect("Failed to close maintenance connection.");

    let connection_pool = PgPool::connect_with(config.connect_options())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database.");
    connection_pool.close().await;
}
