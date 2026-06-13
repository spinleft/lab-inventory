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
        c.database.username = "postgres".to_string();
        c.database.password = Secret::new("password".to_string());
        c.application.port = 0;
        c.application.cookie_secure = false;
        c
    };

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
        test_user: TestUser::generate(),
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

    pub async fn get_units(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/units", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset_category<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/asset-categories", &self.address))
            .json(body)
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

    pub async fn post_location<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/locations", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_asset<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/assets", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_assets(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/assets", &self.address))
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

    pub async fn post_inventory_item<Body>(
        &self,
        body: &Body,
        idempotency_key: &str,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/inventory-items", &self.address))
            .header("Idempotency-Key", idempotency_key)
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_inventory_operation<Body>(
        &self,
        inventory_item_id: Uuid,
        operation: &str,
        body: &Body,
        idempotency_key: &str,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/inventory-items/{inventory_item_id}/{operation}",
                &self.address
            ))
            .header("Idempotency-Key", idempotency_key)
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_inventory_items(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/inventory-items", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_stock_alerts(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/stock-alerts", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_borrow_request_alerts(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/borrow-request-alerts", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_maintenance_record<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/maintenance-records", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_maintenance_records(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/maintenance-records", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_maintenance_record(&self, maintenance_record_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/maintenance-records/{maintenance_record_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_maintenance_record<Body>(
        &self,
        maintenance_record_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!(
                "{}/api/v1/maintenance-records/{maintenance_record_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_maintenance_record(
        &self,
        maintenance_record_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/maintenance-records/{maintenance_record_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_maintenance_schedule<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/maintenance-schedules", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_maintenance_schedules(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/maintenance-schedules", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn patch_maintenance_schedule<Body>(
        &self,
        maintenance_schedule_id: Uuid,
        body: &Body,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .patch(format!(
                "{}/api/v1/maintenance-schedules/{maintenance_schedule_id}",
                &self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_maintenance_schedule(
        &self,
        maintenance_schedule_id: Uuid,
    ) -> reqwest::Response {
        self.api_client
            .delete(format!(
                "{}/api/v1/maintenance-schedules/{maintenance_schedule_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_maintenance_alerts(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/maintenance-alerts", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_attachment<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/attachments", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_attachments(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/attachments", &self.address))
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

    pub async fn post_borrow_request<Body>(
        &self,
        body: &Body,
        idempotency_key: &str,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/api/v1/borrow-requests", &self.address))
            .header("Idempotency-Key", idempotency_key)
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_borrow_requests(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/api/v1/borrow-requests", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_borrow_request(&self, borrow_request_id: Uuid) -> reqwest::Response {
        self.api_client
            .get(format!(
                "{}/api/v1/borrow-requests/{borrow_request_id}",
                &self.address
            ))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_borrow_request_operation<Body>(
        &self,
        borrow_request_id: Uuid,
        operation: &str,
        body: &Body,
        idempotency_key: &str,
    ) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!(
                "{}/api/v1/borrow-requests/{borrow_request_id}/{operation}",
                &self.address
            ))
            .header("Idempotency-Key", idempotency_key)
            .json(body)
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
