use super::borrowing::{parse_borrow_target, respond_federation_borrow};
use super::public_data::{PublicDataError, parse_read_target, respond_public_data};
use super::queries::FederationDatabaseError;
use super::security::{FederationSecurityError, verify_inbound_request};
use super::service::upsert_guest_link;
use crate::configuration::FederationSettings;
use crate::file_storage::FileStorage;
use crate::routes::borrow_requests::BorrowRequestError;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct InboundPath {
    laboratory_id: Uuid,
    tail: Option<String>,
}

#[derive(thiserror::Error)]
pub enum InboundFederationError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for InboundFederationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for InboundFederationError {
    fn status_code(&self) -> StatusCode {
        match self {
            InboundFederationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            InboundFederationError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            InboundFederationError::Forbidden(_) => StatusCode::FORBIDDEN,
            InboundFederationError::NotFound(_) => StatusCode::NOT_FOUND,
            InboundFederationError::ConflictError(_) => StatusCode::CONFLICT,
            InboundFederationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationSecurityError> for InboundFederationError {
    fn from(error: FederationSecurityError) -> Self {
        match error {
            FederationSecurityError::Unauthorized(message) => Self::Unauthorized(message),
            FederationSecurityError::Forbidden(message) => Self::Forbidden(message),
            FederationSecurityError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<FederationDatabaseError> for InboundFederationError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<PublicDataError> for InboundFederationError {
    fn from(error: PublicDataError) -> Self {
        match error {
            PublicDataError::Validation(message) => Self::ValidationError(message),
            PublicDataError::NotFound(message) => Self::NotFound(message),
            PublicDataError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<BorrowRequestError> for InboundFederationError {
    fn from(error: BorrowRequestError) -> Self {
        match error {
            BorrowRequestError::ValidationError(message) => Self::ValidationError(message),
            BorrowRequestError::Forbidden(message) => Self::Forbidden(message),
            BorrowRequestError::NotFound(message) => Self::NotFound(message),
            BorrowRequestError::ConflictError(message) => Self::ConflictError(message),
            BorrowRequestError::UnexpectedError(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Federation inbound GET",
    skip(pool, settings, storage, body, req),
    fields(laboratory_id=tracing::field::Empty, tail=tracing::field::Empty)
)]
pub async fn inbound_get(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    storage: web::Data<FileStorage>,
    path: web::Path<InboundPath>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, InboundFederationError> {
    handle_inbound(pool, settings, storage, path, body, req).await
}

#[tracing::instrument(
    name = "Federation inbound POST",
    skip(pool, settings, storage, body, req),
    fields(laboratory_id=tracing::field::Empty, tail=tracing::field::Empty)
)]
pub async fn inbound_post(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    storage: web::Data<FileStorage>,
    path: web::Path<InboundPath>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, InboundFederationError> {
    handle_inbound(pool, settings, storage, path, body, req).await
}

/// The body is taken as raw [`web::Bytes`] rather than through `web::Json` for
/// two reasons: only one extractor may consume the payload, and the signature
/// covers these exact bytes, so they have to reach [`verify_inbound_request`]
/// untouched before anything tries to interpret them.
async fn handle_inbound(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    storage: web::Data<FileStorage>,
    path: web::Path<InboundPath>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, InboundFederationError> {
    let path = path.into_inner();
    let laboratory_id = path.laboratory_id;
    let tail = path.tail.unwrap_or_default();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("tail", tracing::field::display(&tail));

    // Resolved before the request is authenticated, matching how reads have
    // always behaved here.
    let borrow_target = parse_borrow_target(req.method(), &tail);
    // A write that is not a borrow route is refused outright. Falling back to the
    // read parser would let a POST to any readable path be answered with that
    // path's contents.
    let read_target = match (&borrow_target, req.method()) {
        (Some(_), _) => None,
        (None, &actix_web::http::Method::GET) => Some(parse_read_target(&tail)?),
        (None, _) => {
            return Err(InboundFederationError::NotFound(
                "Federation route not found".into(),
            ));
        }
    };

    let context = verify_inbound_request(&req, &body, &pool, &settings, laboratory_id).await?;
    let caller = upsert_guest_link(&pool, laboratory_id, &context).await?;

    match (borrow_target, read_target) {
        (Some(target), _) => Ok(respond_federation_borrow(
            &pool,
            laboratory_id,
            &caller,
            target,
            req.query_string(),
            &body,
        )
        .await?),
        (None, Some(target)) => {
            Ok(respond_public_data(&pool, &storage, laboratory_id, target, req.query_string())
                .await?)
        }
        (None, None) => Err(InboundFederationError::NotFound(
            "Federation route not found".into(),
        )),
    }
}
