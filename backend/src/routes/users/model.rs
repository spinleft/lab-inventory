use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct UserResponse {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    phone_number: Option<String>,
    user_type: UserTypeResponse,
    laboratory: Option<UserLaboratoryResponse>,
    created_at: DateTime<Utc>,
    last_login_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct UserTypeResponse {
    user_type_id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct UserLaboratoryResponse {
    laboratory_id: Uuid,
    name: String,
}

#[derive(sqlx::FromRow)]
pub(super) struct UserRow {
    pub(super) user_id: Uuid,
    pub(super) username: String,
    pub(super) email: Option<String>,
    pub(super) phone_number: Option<String>,
    pub(super) user_type_id: Option<Uuid>,
    pub(super) user_type_name: Option<String>,
    pub(super) laboratory_id: Option<Uuid>,
    pub(super) laboratory_name: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) last_login_at: Option<DateTime<Utc>>,
}

pub(super) fn create_user_rollback_details(user: &UserRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "user",
            "where": {
                "user_id": user.user_id,
            },
        },
    })
}

pub(super) fn update_user_rollback_details(user: &UserRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "user",
            "where": {
                "user_id": user.user_id,
            },
            "values": {
                "username": &user.username,
                "user_type_id": user.user_type_id,
                "user_type": user.user_type_name.as_deref(),
                "laboratory_id": user.laboratory_id,
                "email": user.email.as_deref(),
                "phone_number": user.phone_number.as_deref(),
            },
        },
    })
}

impl From<UserRow> for UserResponse {
    fn from(row: UserRow) -> Self {
        Self {
            user_id: row.user_id,
            username: row.username,
            email: row.email,
            phone_number: row.phone_number,
            user_type: row
                .user_type_id
                .zip(row.user_type_name)
                .map(|(user_type_id, name)| UserTypeResponse { user_type_id, name })
                .unwrap_or(UserTypeResponse {
                    user_type_id: Uuid::nil(),
                    name: "Unknown".to_string(),
                }),
            laboratory: row
                .laboratory_id
                .zip(row.laboratory_name)
                .map(|(laboratory_id, name)| UserLaboratoryResponse {
                    laboratory_id,
                    name,
                }),
            created_at: row.created_at,
            last_login_at: row.last_login_at,
        }
    }
}

pub(super) async fn fetch_user(pool: &PgPool, user_id: Uuid) -> Result<UserRow, anyhow::Error> {
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT
            users.user_id,
            users.username,
            users.email,
            users.phone_number,
            user_types.user_type_id,
            user_types.name AS user_type_name,
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            users.created_at,
            users.last_login_at
        FROM users
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        WHERE users.user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow!(e))?
    .ok_or(anyhow!("User not found"))
}
