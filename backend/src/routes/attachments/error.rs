use crate::routes::PaginationError;
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;

#[derive(thiserror::Error)]
pub enum AttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            AttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            AttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            AttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            AttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PaginationError> for AttachmentError {
    fn from(error: PaginationError) -> Self {
        Self::ValidationError(error.to_string())
    }
}
