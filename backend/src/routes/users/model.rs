use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
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

#[derive(sqlx::FromRow)]
pub(super) struct DeletedUserRow {
    pub(super) user_id: Uuid,
    pub(super) username: String,
    pub(super) password_hash: String,
    pub(super) email: Option<String>,
    pub(super) phone_number: Option<String>,
    pub(super) user_type_id: Option<Uuid>,
    pub(super) user_type_name: Option<String>,
    pub(super) laboratory_id: Option<Uuid>,
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

pub(super) fn delete_user_rollback_details(user: &DeletedUserRow) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "user",
            "values": {
                "user_id": user.user_id,
                "username": &user.username,
                "password_hash": &user.password_hash,
                "user_type_id": user.user_type_id,
                "user_type": user.user_type_name.as_deref(),
                "laboratory_id": user.laboratory_id,
                "email": user.email.as_deref(),
                "phone_number": user.phone_number.as_deref(),
                "created_at": &user.created_at,
                "last_login_at": user.last_login_at.as_ref(),
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
