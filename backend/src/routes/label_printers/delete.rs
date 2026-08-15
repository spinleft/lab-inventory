use super::model::delete_label_printer_rollback_details;
use super::queries::{
    LabelPrinterDatabaseError, delete_label_printer_from_database, fetch_label_printer_for_update,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::LabelPrinterId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteLabelPrinterError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteLabelPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteLabelPrinterError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteLabelPrinterError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteLabelPrinterError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteLabelPrinterError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteLabelPrinterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LabelPrinterDatabaseError> for DeleteLabelPrinterError {
    fn from(error: LabelPrinterDatabaseError) -> Self {
        match error {
            LabelPrinterDatabaseError::Validation(message) => Self::ConflictError(message),
            LabelPrinterDatabaseError::Conflict(message) => Self::ConflictError(message),
            LabelPrinterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete a label printer",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_id=%printer_id)
)]
pub async fn delete_label_printer(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    printer_id: LabelPrinterId,
) -> Result<HttpResponse, DeleteLabelPrinterError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::LabelPrinter,
        Action::Delete(*printer_id),
    )
    .await?
    {
        return Err(DeleteLabelPrinterError::Forbidden(
            "You are not allowed to delete this label printer.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_label_printer_for_update(&mut transaction, *printer_id)
        .await?
        .ok_or(DeleteLabelPrinterError::NotFound(
            "Label printer not found".into(),
        ))?;
    delete_label_printer_from_database(&mut transaction, existing.printer_id).await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Delete,
        AuditResource::LabelPrinter,
        Some(existing.printer_id),
        delete_label_printer_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete a label printer.")?;

    Ok(HttpResponse::NoContent().finish())
}
