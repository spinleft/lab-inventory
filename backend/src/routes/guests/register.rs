use super::queries::{RegistrationCodeRow, consume_registration_code};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{GuestRegistrationHasher, GuestRegistrationRateLimiter, hash_password};
use crate::domain::{
    GuestRegistrationCode, NewUser, PhoneNumber, UserEmail, UserId, UserName, UserPassword,
    UserType,
};
use crate::routes::users::{UserDatabaseError, store_new_user};
use crate::utils::error_chain_fmt;
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::{StatusCode, header};
use actix_web::middleware::Next;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use secrecy::Secret;
use serde_json::{Value, json};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestRegistrationJsonData {
    username: String,
    password: Secret<String>,
    email: String,
    phone_number: String,
    registration_code: Secret<String>,
}

#[derive(Debug)]
struct GuestRegistration {
    username: UserName,
    password: UserPassword,
    email: UserEmail,
    phone_number: PhoneNumber,
    registration_code: GuestRegistrationCode,
}

impl TryFrom<GuestRegistrationJsonData> for GuestRegistration {
    type Error = String;

    fn try_from(value: GuestRegistrationJsonData) -> Result<Self, Self::Error> {
        Ok(Self {
            username: UserName::parse(value.username)?,
            password: UserPassword::parse(value.password)?,
            email: UserEmail::parse(value.email)?,
            phone_number: PhoneNumber::parse(value.phone_number)?,
            registration_code: GuestRegistrationCode::parse(value.registration_code)?,
        })
    }
}

#[derive(thiserror::Error)]
pub enum GuestRegistrationError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GuestRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GuestRegistrationError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ValidationError(_) => StatusCode::BAD_REQUEST,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::ConflictError(_) => StatusCode::CONFLICT,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({ "error": self.to_string() }))
    }
}

impl From<UserDatabaseError> for GuestRegistrationError {
    fn from(error: UserDatabaseError) -> Self {
        match error {
            UserDatabaseError::Validation(message) => Self::ValidationError(message),
            UserDatabaseError::Conflict(message) => Self::ConflictError(message),
            UserDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

pub async fn enforce_guest_registration_rate_limit(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, actix_web::Error> {
    let Some(peer_ip) = req.peer_addr().map(|address| address.ip()) else {
        return Ok(service_unavailable_response(req));
    };
    let Some(rate_limiter) = req
        .app_data::<web::Data<GuestRegistrationRateLimiter>>()
        .cloned()
    else {
        tracing::error!("Guest registration rate limiter is missing from application data");
        return Ok(service_unavailable_response(req));
    };

    match rate_limiter.check(peer_ip).await {
        Ok(limit) if limit.allowed => next
            .call(req)
            .await
            .map(ServiceResponse::map_into_left_body),
        Ok(limit) => {
            let response = HttpResponse::TooManyRequests()
                .insert_header((header::RETRY_AFTER, limit.retry_after_seconds.to_string()))
                .json(json!({ "error": "Too many registration attempts" }));
            Ok(req.into_response(response).map_into_right_body())
        }
        Err(error) => {
            tracing::error!(error = ?error, "Guest registration rate limiting failed");
            Ok(service_unavailable_response(req))
        }
    }
}

fn service_unavailable_response<B>(req: ServiceRequest) -> ServiceResponse<EitherBody<B>> {
    req.into_response(
        HttpResponse::ServiceUnavailable()
            .json(json!({ "error": "Guest registration is temporarily unavailable" })),
    )
    .map_into_right_body()
}

#[tracing::instrument(name = "Registering a guest", skip(pool, hasher, payload))]
pub async fn register_guest(
    pool: web::Data<PgPool>,
    hasher: web::Data<GuestRegistrationHasher>,
    payload: web::Json<GuestRegistrationJsonData>,
) -> Result<HttpResponse, GuestRegistrationError> {
    let registration = GuestRegistration::try_from(payload.into_inner())
        .map_err(GuestRegistrationError::ValidationError)?;
    let code_hmac = hasher.hash_code(&registration.registration_code);
    let password_hash = hash_password(registration.password.clone().0).await?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to begin guest registration transaction")?;
    let code = consume_registration_code(&mut transaction, &code_hmac)
        .await?
        .ok_or_else(|| {
            GuestRegistrationError::ValidationError(
                "Registration code is invalid or expired".into(),
            )
        })?;
    let new_user = NewUser::new(
        registration.username,
        registration.password,
        UserType::Guest,
        Some(code.laboratory_id.into()),
        Some(registration.email),
        Some(registration.phone_number),
    )
    .map_err(GuestRegistrationError::ValidationError)?;
    let created_user = store_new_user(&mut transaction, new_user, password_hash).await?;
    let registered_user_id = UserId(created_user.user_id);

    record_audit(
        &mut transaction,
        registered_user_id,
        AuditAction::Update,
        AuditResource::GuestRegistrationCode,
        Some(code.registration_code_id),
        consumed_code_audit_details(&code),
    )
    .await?;
    let mut user_details = created_user.audit_details;
    user_details["guest_registration"] = json!({
        "registration_code_id": code.registration_code_id,
        "created_by_user_id": code.created_by_user_id,
        "laboratory_id": code.laboratory_id,
    });
    record_audit(
        &mut transaction,
        registered_user_id,
        AuditAction::Create,
        AuditResource::User,
        Some(created_user.user_id),
        user_details,
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit guest registration transaction")?;

    Ok(HttpResponse::Created().json(created_user.response))
}

fn consumed_code_audit_details(code: &RegistrationCodeRow) -> Value {
    json!({
        "laboratory_id": code.laboratory_id,
        "created_by_user_id": code.created_by_user_id,
        "reason": "consumed",
    })
}

#[cfg(test)]
mod tests {
    use super::{GuestRegistration, GuestRegistrationJsonData};
    use claims::{assert_err, assert_ok};
    use secrecy::Secret;

    #[test]
    fn valid_registration_payload_is_accepted() {
        assert_ok!(GuestRegistration::try_from(GuestRegistrationJsonData {
            username: "guest-user".into(),
            password: Secret::new("password".into()),
            email: "guest@example.com".into(),
            phone_number: "12345678901".into(),
            registration_code: Secret::new("012345".into()),
        }));
    }

    #[test]
    fn every_registration_field_is_validated() {
        for payload in [
            GuestRegistrationJsonData {
                username: "".into(),
                password: Secret::new("password".into()),
                email: "guest@example.com".into(),
                phone_number: "12345678901".into(),
                registration_code: Secret::new("012345".into()),
            },
            GuestRegistrationJsonData {
                username: "guest-user".into(),
                password: Secret::new("short".into()),
                email: "guest@example.com".into(),
                phone_number: "12345678901".into(),
                registration_code: Secret::new("012345".into()),
            },
            GuestRegistrationJsonData {
                username: "guest-user".into(),
                password: Secret::new("password".into()),
                email: "invalid".into(),
                phone_number: "12345678901".into(),
                registration_code: Secret::new("012345".into()),
            },
            GuestRegistrationJsonData {
                username: "guest-user".into(),
                password: Secret::new("password".into()),
                email: "guest@example.com".into(),
                phone_number: "123".into(),
                registration_code: Secret::new("012345".into()),
            },
            GuestRegistrationJsonData {
                username: "guest-user".into(),
                password: Secret::new("password".into()),
                email: "guest@example.com".into(),
                phone_number: "12345678901".into(),
                registration_code: Secret::new("12345".into()),
            },
        ] {
            assert_err!(GuestRegistration::try_from(payload));
        }
    }
}
