//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
//!
//! [`assign_uploaded_attachments`] is the one flow other route modules reach
//! for: assets and inventory items accept attachments inline on create, and go
//! through this module so an attachment is always born the same way. Its error
//! type therefore lives here rather than in a handler module.
use super::model::{AttachmentRow, AttachmentTarget, create_attachment_rollback_details};
use super::queries::{AttachmentDatabaseError, insert_attachment};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, NewAttachment, UserId};
use crate::routes::file_uploads::{ConsumeFileUploadError, consume_file_upload};
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum AssignAttachmentError {
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

impl std::fmt::Debug for AssignAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AssignAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            AssignAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AssignAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            AssignAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            AssignAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            AssignAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ConsumeFileUploadError> for AssignAttachmentError {
    fn from(error: ConsumeFileUploadError) -> Self {
        match error {
            ConsumeFileUploadError::ValidationError(message) => Self::ValidationError(message),
            ConsumeFileUploadError::NotFound(message) => Self::NotFound(message),
            ConsumeFileUploadError::ConflictError(message) => Self::ConflictError(message),
            ConsumeFileUploadError::UnexpectedError(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<AttachmentDatabaseError> for AssignAttachmentError {
    fn from(error: AttachmentDatabaseError) -> Self {
        match error {
            AttachmentDatabaseError::Validation(message) => Self::ValidationError(message),
            AttachmentDatabaseError::Conflict(message) => Self::ConflictError(message),
            AttachmentDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

/// Consumes the given uploads and turns each into an attachment on `target`.
///
/// `expected_laboratory_id` is checked against every upload when supplied. The
/// create endpoints pass it because they already know the laboratory they are
/// writing into and want an explicit error; the assign handlers only hold the
/// target id and pass `None`, relying on the `(asset_id, laboratory_id)` and
/// `(inventory_item_id, laboratory_id)` composite foreign keys to reject uploads
/// from another laboratory.
pub(crate) async fn assign_uploaded_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: UserId,
    target: AttachmentTarget,
    expected_laboratory_id: Option<LaboratoryId>,
    attachments: &[NewAttachment],
) -> Result<Vec<AttachmentRow>, AssignAttachmentError> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    validate_unique_uploads(attachments)?;

    let mut rows = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        // Consuming marks the upload as spent inside this transaction, so the
        // same upload cannot be turned into two attachments.
        let upload = consume_file_upload(transaction, attachment.upload_id).await?;
        if let Some(expected_laboratory_id) = expected_laboratory_id
            && upload.laboratory_id != Uuid::from(expected_laboratory_id)
        {
            return Err(AssignAttachmentError::ValidationError(
                "File upload does not belong to the target laboratory".into(),
            ));
        }

        // An attachment without a name of its own is shown under the name the
        // file was uploaded with.
        let display_name = match attachment.display_name.clone() {
            Some(value) => value.as_ref().to_string(),
            None => upload.original_file_name.clone(),
        };
        let row = insert_attachment(
            transaction,
            actor_user_id,
            &target,
            &upload,
            &display_name,
            attachment.description.as_deref(),
            attachment.is_public,
        )
        .await?;

        record_audit(
            transaction,
            actor_user_id,
            AuditAction::Create,
            AuditResource::Attachment,
            Some(row.attachment_id),
            create_attachment_rollback_details(&row),
        )
        .await?;
        rows.push(row);
    }

    Ok(rows)
}

fn validate_unique_uploads(attachments: &[NewAttachment]) -> Result<(), AssignAttachmentError> {
    let mut seen = HashSet::new();
    for attachment in attachments {
        if !seen.insert(*attachment.upload_id) {
            return Err(AssignAttachmentError::ValidationError(
                "attachments cannot contain duplicate upload_id values".into(),
            ));
        }
    }

    Ok(())
}
