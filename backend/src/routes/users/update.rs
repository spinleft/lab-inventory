use super::model::{UserResponse, UserRow, update_user_rollback_details};
use super::queries::{UserDatabaseError, fetch_user, update_user_in_database};
use crate::access_control::get_actor;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    LaboratoryId, NullableUpdate, PhoneNumber, UpdateUser, UserEmail, UserName, UserType,
};
use crate::domain::{UserId, UserRole};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserJsonData {
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

impl UpdateUserJsonData {
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
        let laboratory_id = self.laboratory_id.map(|id| id.map(Uuid::into)).into();
        let email = NullableUpdate::parse(self.email, UserEmail::parse)?;
        let phone_number = NullableUpdate::parse(self.phone_number, PhoneNumber::parse)?;

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

impl From<UserDatabaseError> for UpdateUserError {
    fn from(error: UserDatabaseError) -> Self {
        match error {
            UserDatabaseError::Validation(message) => Self::ValidationError(message),
            UserDatabaseError::Conflict(message) => Self::ConflictError(message),
            UserDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
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
    payload: web::Json<UpdateUserJsonData>,
) -> Result<HttpResponse, UpdateUserError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateUserError::UnexpectedError)?
        .ok_or(UpdateUserError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let target_user = fetch_user(&pool, *target_user_id).await?;
    let target_user_id = target_user.user_id.into();
    let target_user_type = parse_user_type(&target_user)?;
    let target_laboratory_id = target_user.laboratory_id.map(Uuid::into);
    let target_user_role = UserRole {
        user_type: target_user_type,
        laboratory_id: target_laboratory_id,
    };
    let payload = payload.into_inner();

    if actor.user_id == target_user_id && payload.updates_role_or_laboratory() {
        return Err(UpdateUserError::ValidationError(
            "Users cannot change their own role or laboratory".into(),
        ));
    }
    let update_user = payload
        .into_update_user(target_user_type, target_laboratory_id)
        .map_err(UpdateUserError::ValidationError)?;
    let update_user_role = UserRole {
        user_type: update_user.user_type,
        laboratory_id: update_user.laboratory_id,
    };

    if (actor.user_id != target_user_id)
        && !validate_permission(
            &pool,
            &actor_user_id,
            ResourceType::User,
            Action::UpdateUser(&target_user_role, &update_user_role),
        )
        .await?
    {
        return Err(UpdateUserError::Forbidden(
            "You don't have permission to update this user.".into(),
        ));
    }

    let username = update_user.username;
    let user_type = update_user.user_type;
    let laboratory_id = update_user.laboratory_id;
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
        actor_user_id,
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
