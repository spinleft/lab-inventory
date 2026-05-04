use super::model::{UserResponse, UserRow, fetch_user};
use super::validation::{
    map_database_error, normalize_group, required_text, resolve_target_laboratory,
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
    username: Option<String>,
    password: Option<Secret<String>>,
    group: Option<String>,
    laboratory_id: Option<Uuid>,
    email: Option<String>,
}

#[tracing::instrument(
    name = "Update a user",
    skip(pool, payload),
    fields(actor_user_id=%user_id, target_user_id=%target_user_id)
)]
pub async fn update_user(
    user_id: UserId,
    pool: web::Data<PgPool>,
    target_user_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let target = fetch_user(pool.get_ref(), *target_user_id).await?;
    let group_name = match payload.group.as_deref() {
        Some(group) => normalize_group(group)?,
        None => target.group_name.clone(),
    };
    let laboratory_id = resolve_target_laboratory(
        &actor,
        &group_name,
        payload.laboratory_id.or(target.laboratory_id),
    )?;
    validate_user_management(pool.get_ref(), &actor, &group_name, laboratory_id).await?;

    let username = payload
        .username
        .as_deref()
        .map(|username| required_text(username, "username"))
        .transpose()?;
    let password_hash = match payload.password.clone() {
        Some(password) => Some(
            hash_password(password)
                .await
                .map_err(ApiError::UnexpectedError)?,
        ),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let user = sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET
            username = COALESCE($2, username),
            password_hash = COALESCE($3, password_hash),
            group_id = (SELECT group_id FROM user_groups WHERE name = $4),
            laboratory_id = $5,
            email = COALESCE($6, email)
        WHERE user_id = $1
        RETURNING
            users.user_id,
            users.username,
            users.email,
            (SELECT group_id FROM user_groups WHERE name = $4) AS group_id,
            $4 AS group_name,
            users.laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = users.laboratory_id) AS laboratory_name,
            users.created_at,
            users.last_login_at
        "#,
    )
    .bind(target.user_id)
    .bind(username)
    .bind(password_hash.as_ref().map(|hash| hash.expose_secret()))
    .bind(&group_name)
    .bind(laboratory_id)
    .bind(payload.email.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        user.laboratory_id,
        AuditAction::Update,
        AuditResource::User,
        Some(user.user_id),
        json!({ "username": user.username, "group": user.group_name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(UserResponse::from(user)))
}
