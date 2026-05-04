use crate::authentication::{AuthError, Credentials, validate_credentials};
use crate::session_state::TypedSession;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use secrecy::Secret;
use serde::Serialize;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct JsonData {
    username: String,
    password: Secret<String>,
}

#[derive(Serialize)]
struct MessageResponse {
    message: &'static str,
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        match self {
            LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
            LoginError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": self.to_string()
        }))
    }
}

#[tracing::instrument(
    name = "Log in user",
    skip(json, pool, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    json: web::Json<JsonData>,
    pool: web::Data<PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: json.0.username,
        password: json.0.password,
    };
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));

    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            sqlx::query("UPDATE users SET last_login_at = now() WHERE user_id = $1")
                .bind(user_id)
                .execute(pool.get_ref())
                .await
                .map_err(|e| LoginError::UnexpectedError(e.into()))?;
            session.renew();
            session
                .insert_user_id(user_id)
                .map_err(|e| LoginError::UnexpectedError(e.into()))?;

            Ok(HttpResponse::Ok().json(MessageResponse {
                message: "Login successful",
            }))
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };
            Err(e)
        }
    }
}

pub async fn logout(session: TypedSession) -> HttpResponse {
    session.log_out();
    HttpResponse::Ok().json(MessageResponse {
        message: "Logout successful",
    })
}
