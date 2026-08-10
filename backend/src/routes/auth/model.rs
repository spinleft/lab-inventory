use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// The reply to any auth action that has nothing to return but success.
#[derive(Serialize)]
pub(super) struct MessageResponse {
    pub(super) message: &'static str,
}

#[derive(sqlx::FromRow)]
pub(super) struct CurrentUserRow {
    pub(super) user_id: Uuid,
    pub(super) username: String,
    pub(super) email: Option<String>,
    pub(super) user_type_id: Uuid,
    pub(super) user_type_name: String,
    pub(super) laboratory_id: Option<Uuid>,
    pub(super) laboratory_name: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CurrentUser {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    user_type: CurrentUserType,
    laboratory: Option<CurrentUserLaboratory>,
}

#[derive(Serialize)]
struct CurrentUserType {
    user_type_id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct CurrentUserLaboratory {
    laboratory_id: Uuid,
    name: String,
}

impl From<CurrentUserRow> for CurrentUser {
    fn from(row: CurrentUserRow) -> Self {
        Self {
            user_id: row.user_id,
            username: row.username,
            email: row.email,
            user_type: CurrentUserType {
                user_type_id: row.user_type_id,
                name: row.user_type_name,
            },
            laboratory: row
                .laboratory_id
                .zip(row.laboratory_name)
                .map(|(laboratory_id, name)| CurrentUserLaboratory {
                    laboratory_id,
                    name,
                }),
        }
    }
}

/// The user whose password was just changed, carrying the hash that was
/// replaced.
#[derive(sqlx::FromRow)]
pub(super) struct ChangedPasswordUser {
    pub(super) user_id: Uuid,
    pub(super) previous_password_hash: String,
}

pub(super) fn change_password_rollback_details(user: &ChangedPasswordUser) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "user",
            "where": {
                "user_id": user.user_id,
            },
            "values": {
                "password_hash": &user.previous_password_hash,
            },
        },
    })
}
