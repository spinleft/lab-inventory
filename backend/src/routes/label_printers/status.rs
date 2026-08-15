use super::queries::fetch_label_printer;
use super::service::address_policy;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::configuration::LabelPrintingSettings;
use crate::domain::LabelPrinterId;
use crate::label_printing::status::PrinterStatus;
use crate::label_printing::transport::{self, TransportError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct PrinterStatusResponse {
    #[serde(flatten)]
    status: PrinterStatus,
    /// Whether the loaded stock is what the printer is configured for. A
    /// mismatch is what the print endpoint would refuse on, surfaced here so
    /// the UI can warn before anyone commits a roll to it.
    media_matches_configuration: bool,
    ready: bool,
}

#[derive(thiserror::Error)]
pub enum LabelPrinterStatusError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    PrinterUnreachable(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LabelPrinterStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LabelPrinterStatusError {
    fn status_code(&self) -> StatusCode {
        match self {
            LabelPrinterStatusError::ValidationError(_) => StatusCode::BAD_REQUEST,
            LabelPrinterStatusError::NotFound(_) => StatusCode::NOT_FOUND,
            LabelPrinterStatusError::PrinterUnreachable(_) => StatusCode::BAD_GATEWAY,
            LabelPrinterStatusError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<TransportError> for LabelPrinterStatusError {
    fn from(error: TransportError) -> Self {
        match error {
            TransportError::BlockedAddress(_) | TransportError::BlockedPort(_) => {
                Self::ValidationError(error.to_string())
            }
            other => Self::PrinterUnreachable(other.to_string()),
        }
    }
}

#[tracing::instrument(
    name = "Query label printer status",
    skip(pool, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_id=%printer_id)
)]
pub async fn get_label_printer_status(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    settings: web::Data<LabelPrintingSettings>,
    printer_id: LabelPrinterId,
) -> Result<HttpResponse, LabelPrinterStatusError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::LabelPrinter,
        Action::Read(*printer_id),
    )
    .await?
    {
        return Err(LabelPrinterStatusError::NotFound(
            "Label printer not found".into(),
        ));
    }

    let printer =
        fetch_label_printer(&pool, *printer_id)
            .await?
            .ok_or(LabelPrinterStatusError::NotFound(
                "Label printer not found".into(),
            ))?;

    let status = transport::query_status(&printer.endpoint(), address_policy(&settings)).await?;
    let media_matches_configuration = printer
        .media()
        .map(|media| status.matches(media.spec()))
        .unwrap_or(false);

    Ok(HttpResponse::Ok().json(PrinterStatusResponse {
        ready: status.is_ready(),
        media_matches_configuration,
        status,
    }))
}
