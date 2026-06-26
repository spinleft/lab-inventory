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

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
}

pub async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.application.cookie_secure = false;
        c.attachment_storage.local_root = std::env::temp_dir()
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
        test_user: TestUser {
            user_type: "super_admin".to_string(),
            laboratory_id: None,
            ..TestUser::generate()
        },
        api_client: client,
    };

    test_app.test_user.store(&test_app.db_pool).await;
    test_app
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

    pub async fn post_laboratory<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/laboratories", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratories(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/laboratories", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratory(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}",
                &self.address
            ))
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
        self.api_client
            .patch(format!(
                "{}/api/v1/laboratories/{laboratory_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_laboratory(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/laboratories/{laboratory_id}",
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
        self.api_client
            .post(format!(
                "{}/api/v1/laboratories/{laboratory_id}/asset-categories",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_categories(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/asset-categories",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_categories_under(
        &self,
        laboratory_id: Uuid,
        root_category_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/asset-categories?root_category_id={root_category_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_category(&self, category_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/asset-categories/{category_id}",
                &self.address
            ))
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
        self.api_client
            .patch(format!(
                "{}/api/v1/asset-categories/{category_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset_category(&self, category_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/asset-categories/{category_id}",
                &self.address
            ))
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
        self.api_client
            .post(format!(
                "{}/api/v1/laboratories/{laboratory_id}/asset-parameters",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_parameters(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/asset-parameters",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_parameter(&self, parameter_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/asset-parameters/{parameter_id}",
                &self.address
            ))
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
        self.api_client
            .patch(format!(
                "{}/api/v1/asset-parameters/{parameter_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset_parameter(&self, parameter_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/asset-parameters/{parameter_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset<Body>(&self, laboratory_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/laboratories/{laboratory_id}/assets",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn upload_attachment(
        &self,
        laboratory_id: Uuid,
        file_name: &str,
        mime_type: &str,
        bytes: Vec<u8>,
    ) -> reqwest::Response {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .expect("Invalid attachment MIME type");
        let form = reqwest::multipart::Form::new().part("file", part);
        self.api_client
            .post(format!(
                "{}/api/v1/laboratories/{laboratory_id}/attachment-uploads",
                &self.address
            ))
            .multipart(form)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_attachment_upload(&self, upload_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/attachment-uploads/{upload_id}",
                &self.address
            ))
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
        self.api_client
            .post(format!(
                "{}/api/v1/assets/{asset_id}/attachments",
                &self.address
            ))
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
        self.api_client
            .post(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}/attachments",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_attachments(&self, asset_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/assets/{asset_id}/attachments",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_item_attachments(
        &self,
        inventory_item_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}/attachments",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_laboratory_attachments(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/attachments",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/attachments/{attachment_id}",
                &self.address
            ))
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
        self.api_client
            .patch(format!(
                "{}/api/v1/attachments/{attachment_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/attachments/{attachment_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn download_attachment(&self, attachment_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/attachments/{attachment_id}/download",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_assets(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/assets",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_assets_with_query(
        &self,
        laboratory_id: Uuid,
        query: &str,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/assets?{query}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset(&self, asset_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/assets/{asset_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_asset_with_query(&self, asset_id: Uuid, query: &str) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/assets/{asset_id}?{query}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_asset<Body>(&self, asset_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/assets/{asset_id}", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_asset(&self, asset_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!("{}/api/v1/assets/{asset_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_inventory_items<Body>(&self, asset_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/assets/{asset_id}/inventory-items",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_items(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/inventory-items",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_items_with_query(
        &self,
        laboratory_id: Uuid,
        query: &str,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/inventory-items?{query}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_item(&self, inventory_item_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}",
                &self.address
            ))
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
        self.api_client
            .patch(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_inventory_items_batch<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/inventory-items/batch", &self.address))
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
        self.api_client
            .post(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}/split",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn merge_inventory_items<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/inventory-items/merge", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_inventory_item(&self, inventory_item_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn batch_delete_inventory_items<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/inventory-items/batch-delete",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_unit<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/units", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_units(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/units", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_unit(&self, unit_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/units/{unit_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_unit<Body>(&self, unit_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/units/{unit_id}", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_unit(&self, unit_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!("{}/api/v1/units/{unit_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_location<Body>(&self, laboratory_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/laboratories/{laboratory_id}/locations",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_locations(&self, laboratory_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/locations",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_locations_under(
        &self,
        laboratory_id: Uuid,
        root_location_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/laboratories/{laboratory_id}/locations?root_location_id={root_location_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_location(&self, location_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/locations/{location_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_location<Body>(&self, location_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/locations/{location_id}", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_location(&self, location_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!("{}/api/v1/locations/{location_id}", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_user<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/users", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_user<Body>(&self, user_id: Uuid, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!("{}/api/v1/users/{user_id}", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_user(&self, user_id: Uuid) -> reqwest::Response {
        self.api_client
            .delete(format!("{}/api/v1/users/{user_id}", &self.address))
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

async fn configure_database(config: &DatabaseSettings) -> PgPool {
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

    let connection_pool = PgPool::connect_with(config.connect_options())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database.");
    connection_pool
}
