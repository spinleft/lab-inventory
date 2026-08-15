use super::model::print_labels_details;
use super::queries::fetch_label_printer;
use super::service::{
    MAX_LABELS_PER_REQUEST, PrintError, RequestedPage, address_policy, build_pages, check_ready,
    encode, max_page_bytes,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::LabelPrintingSettings;
use crate::domain::LabelPrinterId;
use crate::label_printing::transport::{self, TransportError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageJsonData {
    width_dots: u16,
    height_dots: u16,
    /// One bit per dot, rows packed MSB-first, a set bit meaning a black dot.
    bitmap_base64: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    pages: Vec<PageJsonData>,
    copies: Option<u32>,
}

#[derive(Serialize)]
struct PrintLabelsResponse {
    labels_printed: usize,
}

#[derive(thiserror::Error)]
pub enum PrintLabelsError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    /// The printer answered, but is not in a state where printing would do the
    /// right thing. Distinct from a validation error because nothing about the
    /// request needs to change — the printer does.
    #[error("{0}")]
    PrinterNotReady(String),
    #[error("{0}")]
    PrinterUnreachable(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PrintLabelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PrintLabelsError {
    fn status_code(&self) -> StatusCode {
        match self {
            PrintLabelsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            PrintLabelsError::Forbidden(_) => StatusCode::FORBIDDEN,
            PrintLabelsError::NotFound(_) => StatusCode::NOT_FOUND,
            PrintLabelsError::PrinterNotReady(_) => StatusCode::CONFLICT,
            PrintLabelsError::PrinterUnreachable(_) => StatusCode::BAD_GATEWAY,
            PrintLabelsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PrintError> for PrintLabelsError {
    fn from(error: PrintError) -> Self {
        match error {
            PrintError::Validation(message) => Self::ValidationError(message),
            PrintError::MediaMismatch { .. } | PrintError::NotReady(_) => {
                Self::PrinterNotReady(error.to_string())
            }
            PrintError::Transport(error) => error.into(),
            PrintError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<TransportError> for PrintLabelsError {
    fn from(error: TransportError) -> Self {
        match error {
            // A blocked address is a configuration mistake, not a network fault.
            TransportError::BlockedAddress(_) | TransportError::BlockedPort(_) => {
                Self::ValidationError(error.to_string())
            }
            other => Self::PrinterUnreachable(other.to_string()),
        }
    }
}

#[tracing::instrument(
    name = "Print labels",
    skip(pool, payload, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_id=%printer_id)
)]
pub async fn print_labels(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    settings: web::Data<LabelPrintingSettings>,
    printer_id: LabelPrinterId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, PrintLabelsError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::LabelPrinter,
        Action::Read(*printer_id),
    )
    .await?
    {
        return Err(PrintLabelsError::NotFound("Label printer not found".into()));
    }

    let printer = fetch_label_printer(&pool, *printer_id)
        .await?
        .ok_or(PrintLabelsError::NotFound("Label printer not found".into()))?;
    let media = printer.require_media()?;

    let payload = payload.into_inner();
    let copies = payload.copies.unwrap_or(1);
    if payload.pages.len() > MAX_LABELS_PER_REQUEST {
        return Err(PrintLabelsError::ValidationError(format!(
            "A print request may contain at most {MAX_LABELS_PER_REQUEST} labels."
        )));
    }

    let page_limit = max_page_bytes(media);
    let requested = payload
        .pages
        .into_iter()
        .map(|page| decode_page(page, page_limit))
        .collect::<Result<Vec<_>, _>>()?;

    let pages = build_pages(media, requested, copies)?;
    let labels_printed = pages.len();
    let job = encode(media, printer.auto_cut, &pages)?;

    // Status and job share one connection, so the stock cannot be swapped
    // between the check and the print.
    let mut connection = transport::open(&printer.endpoint(), address_policy(&settings)).await?;
    let status = connection.request_status().await?;
    check_ready(&status, media)?;
    connection.write_job(&job).await?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Print,
        AuditResource::LabelPrinter,
        Some(printer.printer_id),
        print_labels_details(&printer, pages.len() / copies.max(1) as usize, copies),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to record a label print.")?;

    Ok(HttpResponse::Ok().json(PrintLabelsResponse { labels_printed }))
}

/// Decodes a page's bitmap, refusing anything too large to be a label before
/// allocating for it.
fn decode_page(page: PageJsonData, limit: usize) -> Result<RequestedPage, PrintLabelsError> {
    // Base64 expands by 4/3, so this bounds the decode without doing it.
    if page.bitmap_base64.len() / 4 * 3 > limit {
        return Err(PrintLabelsError::ValidationError(
            "Label bitmap is larger than the printer can accept.".into(),
        ));
    }

    let bitmap = STANDARD
        .decode(page.bitmap_base64.as_bytes())
        .map_err(|_| {
            PrintLabelsError::ValidationError("Label bitmap is not valid base64.".into())
        })?;

    Ok(RequestedPage {
        width_dots: page.width_dots,
        height_dots: page.height_dots,
        bitmap,
    })
}
