use super::model::{UserResponse, create_user_rollback_details};
use super::queries::{UserDatabaseError, insert_user};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::hash_password;
use crate::domain::{NewUser, PhoneNumber, UserEmail, UserName, UserPassword, UserType};
use crate::domain::{UserId, UserRole};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use secrecy::Secret;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUserJsonData {
    username: String,
    password: Secret<String>,
    user_type: String,
    laboratory_id: Option<Uuid>,
    email: Option<String>,
    phone_number: Option<String>,
}

impl TryFrom<CreateUserJsonData> for NewUser {
    type Error = String;

    fn try_from(value: CreateUserJsonData) -> Result<Self, Self::Error> {
        let username = UserName::parse(value.username)?;
        let password = UserPassword::parse(value.password)?;
        let user_type = UserType::parse(&value.user_type)?;
        let laboratory_id = value.laboratory_id.map(Uuid::into);
        let email = value.email.map(UserEmail::parse).transpose()?;
        let phone_number = value.phone_number.map(PhoneNumber::parse).transpose()?;

        NewUser::new(
            username,
            password,
            user_type,
            laboratory_id,
            email,
            phone_number,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CreateUserJsonData;
    use super::NewUser;
    use claims::{assert_err, assert_ok};
    use secrecy::Secret;
    use uuid::Uuid;

    #[test]
    fn valid_json_data_is_converted_to_new_user_successfully() {
        let json_data = CreateUserJsonData {
            username: "testuser".into(),
            password: Secret::new("P@ssw0rd".into()),
            user_type: "lab_admin".into(),
            laboratory_id: Some(Uuid::new_v4()),
            email: Some("testuser@example.com".into()),
            phone_number: Some("12345678901".into()),
        };

        assert_ok!(NewUser::try_from(json_data));
    }

    #[test]
    fn invalid_json_data_is_rejected() {
        // Missing laboratory_id for a lab_admin
        let json_data = CreateUserJsonData {
            username: "testuser".into(),
            password: Secret::new("P@ssw0rd".into()),
            user_type: "lab_admin".into(),
            laboratory_id: None,
            email: Some("testuser@example.com".into()),
            phone_number: Some("12345678901".into()),
        };

        assert_err!(NewUser::try_from(json_data));
    }
}

#[derive(thiserror::Error)]
pub enum CreateUserError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateUserError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateUserError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateUserError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateUserError::ConflictError(_) => StatusCode::CONFLICT,
            CreateUserError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<UserDatabaseError> for CreateUserError {
    fn from(error: UserDatabaseError) -> Self {
        match error {
            UserDatabaseError::Validation(message) => Self::ValidationError(message),
            UserDatabaseError::Conflict(message) => Self::ConflictError(message),
            UserDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Creating a user",
    skip(pool, payload),
    fields(
        actor_user_id=%actor_user_id,
        username=%payload.username,
        user_type=%payload.user_type,
    )
)]
pub async fn create_user(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<CreateUserJsonData>,
) -> Result<HttpResponse, CreateUserError> {
    let new_user =
        NewUser::try_from(payload.into_inner()).map_err(CreateUserError::ValidationError)?;
    let new_user_role = UserRole {
        user_type: new_user.user_type,
        laboratory_id: new_user.laboratory_id,
    };
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::User,
        Action::CreateUser(&new_user_role),
    )
    .await?
    {
        return Err(CreateUserError::Forbidden(
            "You don't have permission to create this user.".into(),
        ));
    }

    let password_hash = hash_password(new_user.password.clone().0).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let created_user = insert_user(&mut transaction, new_user, password_hash).await?;
    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Create,
        AuditResource::User,
        Some(created_user.user_id),
        create_user_rollback_details(&created_user),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new user.")?;
    Ok(HttpResponse::Created().json(UserResponse::from(created_user)))
}
