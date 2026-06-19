use super::model::{UserResponse, UserRow, fetch_user, update_user_rollback_details};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::domain::{
    LaboratoryId, NullableUpdate, PhoneNumber, UpdateUser, UserEmail, UserName, UserType,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    username: Option<String>,
    user_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    laboratory_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    phone_number: Option<Option<String>>,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl JsonData {
    fn updates_role_or_laboratory(&self) -> bool {
        self.user_type.is_some() || self.laboratory_id.is_some()
    }

    fn into_update_user(
        self,
        current_user_type: UserType,
        current_laboratory_id: Option<LaboratoryId>,
    ) -> Result<UpdateUser, String> {
        let username = self.username.map(UserName::parse).transpose()?;
        let user_type = self
            .user_type
            .map(|user_type| UserType::parse(&user_type))
            .transpose()?;
        let laboratory_id = parse_nullable_update(self.laboratory_id, LaboratoryId::parse)?;
        let email = parse_nullable_update(self.email, UserEmail::parse)?;
        let phone_number = parse_nullable_update(self.phone_number, PhoneNumber::parse)?;

        UpdateUser::new(
            username,
            user_type,
            laboratory_id,
            email,
            phone_number,
            current_user_type,
            current_laboratory_id,
        )
    }
}

fn parse_nullable_update<T, V>(
    value: Option<Option<V>>,
    parse: impl FnOnce(V) -> Result<T, String>,
) -> Result<NullableUpdate<T>, String> {
    match value {
        Some(Some(value)) => parse(value).map(NullableUpdate::Set),
        Some(None) => Ok(NullableUpdate::Clear),
        None => Ok(NullableUpdate::Unchanged),
    }
}

#[derive(thiserror::Error)]
pub enum UpdateUserError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UpdateUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateUserError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateUserError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateUserError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateUserError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateUserError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Update a user",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, target_user_id=%target_user_id)
)]
pub async fn update_user(
    pool: web::Data<PgPool>,
    actor_user_id: UserId,
    target_user_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateUserError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateUserError::UnexpectedError)?
        .ok_or(UpdateUserError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let target_user = fetch_user(&pool, *target_user_id).await?;
    let target_user_id =
        UserId::parse(target_user.user_id).map_err(UpdateUserError::ValidationError)?;
    let target_user_type = parse_user_type(&target_user)?;
    let target_laboratory_id = parse_laboratory_id(target_user.laboratory_id)?;
    let payload = payload.into_inner();

    if actor.user_id == target_user_id && payload.updates_role_or_laboratory() {
        return Err(UpdateUserError::ValidationError(
            "Users cannot change their own role or laboratory".into(),
        ));
    }

    let update_user = payload
        .into_update_user(target_user_type, target_laboratory_id)
        .map_err(UpdateUserError::ValidationError)?;

    let username = update_user.username;
    let user_type = update_user.user_type;
    let laboratory_id = update_user.laboratory_id;
    validate_user_update_permission(
        &actor,
        target_user_id,
        target_user_type,
        target_laboratory_id,
        user_type,
        laboratory_id,
    )?;

    let email = resolve_nullable_string_update(update_user.email, target_user.email.clone());
    let phone_number =
        resolve_nullable_string_update(update_user.phone_number, target_user.phone_number.clone());
    let user_type_name = user_type.to_string();

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let user = update_user_in_database(
        &mut transaction,
        target_user.user_id,
        username.as_ref().map(|username| username.as_ref()),
        &user_type_name,
        laboratory_id.map(Uuid::from),
        email.as_deref(),
        phone_number.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::User,
        Some(user.user_id),
        update_user_rollback_details(&target_user),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update a user.")?;

    Ok(HttpResponse::Ok().json(UserResponse::from(user)))
}

async fn update_user_in_database(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    username: Option<&str>,
    user_type_name: &str,
    laboratory_id: Option<Uuid>,
    email: Option<&str>,
    phone_number: Option<&str>,
) -> Result<UserRow, UpdateUserError> {
    sqlx::query_as!(
        UserRow,
        r#"
        WITH updated_user AS (
            UPDATE users
            SET
                username = COALESCE($2, username),
                user_type_id = (SELECT user_type_id FROM user_types WHERE name = $3),
                laboratory_id = $4,
                email = $5,
                phone_number = $6
            WHERE user_id = $1
            RETURNING
                users.user_id,
                users.username,
                users.email,
                users.phone_number,
                users.user_type_id,
                users.laboratory_id,
                users.created_at,
                users.last_login_at
        )
        SELECT
            updated_user.user_id,
            updated_user.username,
            updated_user.email,
            updated_user.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            updated_user.created_at,
            updated_user.last_login_at
        FROM updated_user
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
        user_id,
        username,
        user_type_name,
        laboratory_id,
        email,
        phone_number,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn parse_user_type(user: &UserRow) -> Result<UserType, UpdateUserError> {
    UserType::parse(
        user.user_type_name
            .as_ref()
            .ok_or(UpdateUserError::ValidationError(
                "User type is required".into(),
            ))?,
    )
    .map_err(UpdateUserError::ValidationError)
}

fn parse_laboratory_id(
    laboratory_id: Option<Uuid>,
) -> Result<Option<LaboratoryId>, UpdateUserError> {
    laboratory_id
        .map(|id| LaboratoryId::parse(id).map_err(UpdateUserError::ValidationError))
        .transpose()
}

fn validate_user_update_permission(
    actor: &Actor,
    target_user_id: UserId,
    target_user_type: UserType,
    target_laboratory_id: Option<LaboratoryId>,
    user_type: UserType,
    laboratory_id: Option<LaboratoryId>,
) -> Result<(), UpdateUserError> {
    if actor.user_id == target_user_id {
        return Ok(());
    }

    if actor.can_manage_user(target_user_type, target_laboratory_id)
        && actor.can_manage_user(user_type, laboratory_id)
    {
        Ok(())
    } else {
        Err(UpdateUserError::Forbidden(
            "You don't have permission to update this user.".into(),
        ))
    }
}

fn resolve_nullable_string_update<T>(
    update: NullableUpdate<T>,
    current: Option<String>,
) -> Option<String>
where
    T: Into<String>,
{
    match update {
        NullableUpdate::Unchanged => current,
        NullableUpdate::Set(value) => Some(value.into()),
        NullableUpdate::Clear => None,
    }
}

pub fn map_database_error(error: sqlx::Error) -> UpdateUserError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return UpdateUserError::ConflictError("User already exists".into()),
            Some("23503") => {
                return UpdateUserError::ValidationError("Invalid referenced record".into());
            }
            _ => {}
        }
    }
    UpdateUserError::UnexpectedError(error.into())
}
