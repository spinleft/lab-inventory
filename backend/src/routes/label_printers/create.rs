use super::model::{LabelPrinterResponse, create_label_printer_rollback_details};
use super::queries::{LabelPrinterDatabaseError, insert_label_printer};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    LabelPrinterHost, LabelPrinterMedia, LabelPrinterName, NewLabelPrinter,
};
use crate::label_printing::DEFAULT_PRINTER_PORT;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    name: String,
    host: String,
    port: Option<i32>,
    model: Option<String>,
    media_kind: String,
    media_width_mm: i32,
    media_length_mm: Option<i32>,
    auto_cut: Option<bool>,
}

impl TryFrom<JsonData> for NewLabelPrinter {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::new(
            LabelPrinterName::parse(value.name)?,
            LabelPrinterHost::parse(value.host)?,
            value.port.unwrap_or(i32::from(DEFAULT_PRINTER_PORT)),
            value.model.unwrap_or_else(|| "QL-820NWBc".into()),
            LabelPrinterMedia::parse(
                &value.media_kind,
                value.media_width_mm,
                value.media_length_mm,
            )?,
            value.auto_cut.unwrap_or(true),
        )
    }
}

#[derive(thiserror::Error)]
pub enum CreateLabelPrinterError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateLabelPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateLabelPrinterError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateLabelPrinterError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateLabelPrinterError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateLabelPrinterError::ConflictError(_) => StatusCode::CONFLICT,
            CreateLabelPrinterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LabelPrinterDatabaseError> for CreateLabelPrinterError {
    fn from(error: LabelPrinterDatabaseError) -> Self {
        match error {
            LabelPrinterDatabaseError::Validation(message) => Self::ValidationError(message),
            LabelPrinterDatabaseError::Conflict(message) => Self::ConflictError(message),
            LabelPrinterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create a label printer",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_name=%payload.name)
)]
pub async fn create_label_printer(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateLabelPrinterError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::LabelPrinter,
        Action::Create(*laboratory_id),
    )
    .await?
    {
        return Err(CreateLabelPrinterError::Forbidden(
            "You don't have permission to register label printers.".into(),
        ));
    }

    let new_printer = NewLabelPrinter::try_from(payload.into_inner())
        .map_err(CreateLabelPrinterError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let printer = insert_label_printer(&mut transaction, laboratory_id, &new_printer).await?;
    record_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        AuditResource::LabelPrinter,
        Some(printer.printer_id),
        create_label_printer_rollback_details(&printer),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new label printer.")?;

    Ok(HttpResponse::Created().json(LabelPrinterResponse::from(printer)))
}
