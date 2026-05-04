use crate::helpers::TestApp;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use sqlx::PgPool;
use uuid::Uuid;

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
    pub group: String,
    pub laboratory_id: Option<Uuid>,
}

impl TestUser {
    pub fn generate() -> Self {
        Self::generate_with_group("system_admin", None)
    }

    pub fn generate_with_group(group: &str, laboratory_id: Option<Uuid>) -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
            group: group.to_string(),
            laboratory_id,
        }
    }

    pub async fn login(&self, app: &TestApp) {
        let response = app
            .post_login(&serde_json::json!({
                "username": &self.username,
                "password": &self.password,
            }))
            .await;
        assert_eq!(response.status().as_u16(), 200);
    }

    pub async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO users (user_id, username, password_hash, group_id, laboratory_id)
            SELECT $1, $2, $3, group_id, $4
            FROM user_groups
            WHERE name = $5
            "#,
        )
        .bind(self.user_id)
        .bind(&self.username)
        .bind(password_hash)
        .bind(self.laboratory_id)
        .bind(&self.group)
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}
