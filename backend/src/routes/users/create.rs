use super::model::{UserResponse, UserRow};
use super::validation::{
    map_database_error, normalize_user_type, required_text, resolve_target_laboratory,
    validate_user_management,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor, hash_password};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    username: String,
    password: Secret<String>,
    user_type: String,
    laboratory_id: Option<Uuid>,
    email: Option<String>,
}

#[tracing::instrument(
    name = "Create a user",
    skip(pool, payload),
    fields(actor_user_id=%user_id, username=tracing::field::Empty)
)]
pub async fn create_user(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let username = required_text(&payload.username, "username")?;
    tracing::Span::current().record("username", tracing::field::display(username));
    let user_type_name = normalize_user_type(&payload.user_type)?;
    let laboratory_id = resolve_target_laboratory(&actor, &user_type_name, payload.laboratory_id)?;
    validate_user_management(pool.get_ref(), &actor, &user_type_name, laboratory_id).await?;

    let password_hash = hash_password(payload.password.clone())
        .await
        .map_err(ApiError::UnexpectedError)?;
    let new_user_id = Uuid::new_v4();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let user = sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (user_id, username, password_hash, user_type_id, laboratory_id, email)
        SELECT $1, $2, $3, user_types.user_type_id, $4, $5
        FROM user_types
        WHERE user_types.name = $6
        RETURNING
            users.user_id,
            users.username,
            users.email,
            (SELECT user_type_id FROM user_types WHERE name = $6) AS user_type_id,
            $6 AS user_type_name,
            users.laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = users.laboratory_id) AS laboratory_name,
            users.created_at,
            users.last_login_at
        "#,
    )
    .bind(new_user_id)
    .bind(username)
    .bind(password_hash.expose_secret())
    .bind(laboratory_id)
    .bind(payload.email.as_deref())
    .bind(&user_type_name)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        user.laboratory_id,
        AuditAction::Create,
        AuditResource::User,
        Some(user.user_id),
        json!({ "username": user.username, "user_type": user.user_type_name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(UserResponse::from(user)))
}
