use super::model::{UserResponse, create_user_rollback_details};
use super::queries::{UserDatabaseError, insert_user};
use crate::domain::NewUser;
use secrecy::Secret;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(in crate::routes) struct StoredNewUser {
    pub(in crate::routes) user_id: Uuid,
    pub(in crate::routes) response: UserResponse,
    pub(in crate::routes) audit_details: Value,
}

/// Stores a validated user and packages the public response and rollback audit
/// data without exposing user query rows outside this module.
pub(in crate::routes) async fn store_new_user(
    transaction: &mut Transaction<'_, Postgres>,
    new_user: NewUser,
    password_hash: Secret<String>,
) -> Result<StoredNewUser, UserDatabaseError> {
    let row = insert_user(transaction, new_user, password_hash).await?;
    let user_id = row.user_id;
    let audit_details = create_user_rollback_details(&row);
    let response = UserResponse::from(row);

    Ok(StoredNewUser {
        user_id,
        response,
        audit_details,
    })
}
