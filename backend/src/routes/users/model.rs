use crate::utils::ApiError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct UserResponse {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    group: UserGroupResponse,
    laboratory: Option<UserLaboratoryResponse>,
    created_at: DateTime<Utc>,
    last_login_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct UserGroupResponse {
    group_id: Uuid,
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
    pub(super) group_id: Uuid,
    pub(super) group_name: String,
    pub(super) laboratory_id: Option<Uuid>,
    pub(super) laboratory_name: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) last_login_at: Option<DateTime<Utc>>,
}

impl From<UserRow> for UserResponse {
    fn from(row: UserRow) -> Self {
        Self {
            user_id: row.user_id,
            username: row.username,
            email: row.email,
            group: UserGroupResponse {
                group_id: row.group_id,
                name: row.group_name,
            },
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

pub(super) const USER_SELECT: &str = r#"
    SELECT
        users.user_id,
        users.username,
        users.email,
        user_groups.group_id,
        user_groups.name AS group_name,
        laboratories.laboratory_id,
        laboratories.name AS laboratory_name,
        users.created_at,
        users.last_login_at
    FROM users
    INNER JOIN user_groups USING (group_id)
    LEFT JOIN laboratories USING (laboratory_id)
"#;

pub(super) async fn fetch_user(pool: &PgPool, user_id: Uuid) -> Result<UserRow, ApiError> {
    sqlx::query_as::<_, UserRow>(&format!("{USER_SELECT} WHERE users.user_id = $1"))
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}
