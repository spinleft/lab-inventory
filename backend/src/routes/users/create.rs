use super::model::{UserResponse, UserRow, create_user_rollback_details};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::hash_password;
use crate::domain::UserId;
use crate::domain::{
    LaboratoryId, NewUser, PhoneNumber, UserEmail, UserName, UserPassword, UserType,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    username: String,
    password: Secret<String>,
    user_type: String,
    laboratory_id: Option<Uuid>,
    email: Option<String>,
    phone_number: Option<String>,
}

impl TryFrom<JsonData> for NewUser {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let username = UserName::parse(value.username)?;
        let password = UserPassword::parse(value.password)?;
        let user_type = UserType::parse(&value.user_type)?;
        let laboratory_id = value.laboratory_id.map(LaboratoryId::parse).transpose()?;
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
    use super::JsonData;
    use super::NewUser;
    use claims::{assert_err, assert_ok};
    use secrecy::Secret;
    use uuid::Uuid;

    #[test]
    fn valid_json_data_is_converted_to_new_user_successfully() {
        let json_data = JsonData {
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
        let json_data = JsonData {
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

#[tracing::instrument(
    name = "Creating a user",
    skip(pool, payload),
    fields(
        actor_user_id=%user_id,
        username=%payload.username,
        user_type=%payload.user_type,
    )
)]
pub async fn create_user(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateUserError> {
    let actor = get_actor(&pool, user_id)
        .await
        .map_err(CreateUserError::UnexpectedError)?
        .ok_or(CreateUserError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let new_user =
        NewUser::try_from(payload.into_inner()).map_err(CreateUserError::ValidationError)?;
    validate_create_permission(&actor, &new_user)?;

    let password_hash = hash_password(new_user.password.clone().0).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let created_user = insert_new_user(&mut transaction, new_user, password_hash).await?;
    record_audit(
        &mut transaction,
        &actor,
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

fn validate_create_permission(actor: &Actor, new_user: &NewUser) -> Result<(), CreateUserError> {
    if actor.can_manage_user(new_user.user_type, new_user.laboratory_id) {
        Ok(())
    } else {
        Err(CreateUserError::Forbidden(
            "You don't have permission to create this user.".into(),
        ))
    }
}

#[tracing::instrument(
    name = "Saving new user in the database",
    skip(new_user, password_hash, transaction)
)]
async fn insert_new_user(
    transaction: &mut Transaction<'_, Postgres>,
    new_user: NewUser,
    password_hash: Secret<String>,
) -> Result<UserRow, CreateUserError> {
    let new_user_id = Uuid::new_v4();
    let user_type_name = new_user.user_type.to_string();
    let laboratory_id = new_user.laboratory_id.map(Uuid::from);
    let email = new_user.email.map(String::from);
    let phone_number = new_user.phone_number.map(String::from);

    sqlx::query_as!(
        UserRow,
        r#"
        WITH inserted_user AS (
            INSERT INTO users (user_id, username, password_hash, user_type_id, laboratory_id, email, phone_number)
            SELECT $1, $2, $3, user_types.user_type_id, $4, $5, $6
            FROM user_types
            WHERE user_types.name = $7
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
            inserted_user.user_id,
            inserted_user.username,
            inserted_user.email,
            inserted_user.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            inserted_user.created_at,
            inserted_user.last_login_at
        FROM inserted_user
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
        new_user_id,
        new_user.username.as_ref(),
        password_hash.expose_secret(),
        laboratory_id,
        email,
        phone_number,
        &user_type_name,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> CreateUserError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("users_username_key")) => {
                return CreateUserError::ConflictError("Username already exists".into());
            }
            (Some("23505"), Some("users_email_key")) => {
                return CreateUserError::ConflictError("Email already exists".into());
            }
            (Some("23505"), Some("users_phone_number_key")) => {
                return CreateUserError::ConflictError("Phone number already exists".into());
            }
            (Some("23505"), _) => {
                return CreateUserError::ConflictError("User already exists".into());
            }
            (Some("23503"), Some("users_laboratory_id_fkey")) => {
                return CreateUserError::ValidationError("Invalid laboratory".into());
            }
            (Some("23503"), Some("users_user_type_id_fkey")) => {
                return CreateUserError::ValidationError("Invalid user type".into());
            }
            (Some("23503"), _) => {
                return CreateUserError::ValidationError("Invalid referenced record".into());
            }
            _ => {}
        }
    }
    CreateUserError::UnexpectedError(error.into())
}
