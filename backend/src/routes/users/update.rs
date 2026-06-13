use super::model::{UserResponse, UserRow, fetch_user};
use super::validation::{
    map_database_error, normalize_user_type, required_secret_text, required_text,
    resolve_target_laboratory, validate_user_management,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor, hash_password};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    username: Option<String>,
    password: Option<Secret<String>>,
    user_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    laboratory_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    email: Option<Option<String>>,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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
    if actor.user_id == target.user_id
        && (payload.user_type.is_some() || payload.laboratory_id.is_some())
    {
        return Err(ApiError::BadRequest(
            "Users cannot change their own role or laboratory".into(),
        ));
    }
    let user_type_name = match payload.user_type.as_deref() {
        Some(user_type) => normalize_user_type(user_type)?,
        None => target.user_type_name.clone(),
    };
    let laboratory_id = resolve_target_laboratory(
        &actor,
        &user_type_name,
        payload.laboratory_id.unwrap_or(target.laboratory_id),
    )?;
    validate_user_management(pool.get_ref(), &actor, &user_type_name, laboratory_id).await?;

    let username = payload
        .username
        .as_deref()
        .map(|username| required_text(username, "username"))
        .transpose()?;
    let password_hash = match payload.password.clone() {
        Some(password) => {
            required_secret_text(&password, "password")?;
            Some(
                hash_password(password)
                    .await
                    .map_err(ApiError::UnexpectedError)?,
            )
        }
        None => None,
    };
    let email = payload
        .email
        .clone()
        .unwrap_or_else(|| target.email.clone());

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
            user_type_id = (SELECT user_type_id FROM user_types WHERE name = $4),
            laboratory_id = $5,
            email = $6
        WHERE user_id = $1
        RETURNING
            users.user_id,
            users.username,
            users.email,
            (SELECT user_type_id FROM user_types WHERE name = $4) AS user_type_id,
            $4 AS user_type_name,
            users.laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = users.laboratory_id) AS laboratory_name,
            users.created_at,
            users.last_login_at
        "#,
    )
    .bind(target.user_id)
    .bind(username)
    .bind(password_hash.as_ref().map(|hash| hash.expose_secret()))
    .bind(&user_type_name)
    .bind(laboratory_id)
    .bind(email.as_deref())
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
        json!({ "username": user.username, "user_type": user.user_type_name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(UserResponse::from(user)))
}
