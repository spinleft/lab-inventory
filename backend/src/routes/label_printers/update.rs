use super::model::{LabelPrinterResponse, update_label_printer_rollback_details};
use super::queries::{
    LabelPrinterDatabaseError, fetch_label_printer_for_update, update_label_printer_in_database,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    LabelPrinterHost, LabelPrinterId, LabelPrinterMedia, LabelPrinterName, UpdateLabelPrinter,
    validate_model, validate_port,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    name: Option<String>,
    host: Option<String>,
    port: Option<i32>,
    model: Option<String>,
    media_kind: Option<String>,
    media_width_mm: Option<i32>,
    media_length_mm: Option<i32>,
    auto_cut: Option<bool>,
}

impl TryFrom<JsonData> for UpdateLabelPrinter {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        // Media is one value, not three fields. Changing the kind without the
        // width would leave the printer describing stock that does not exist,
        // so the three are accepted only as a set.
        let media = match (value.media_kind, value.media_width_mm) {
            (Some(kind), Some(width)) => Some(LabelPrinterMedia::parse(
                &kind,
                width,
                value.media_length_mm,
            )?),
            (None, None) if value.media_length_mm.is_none() => None,
            _ => {
                return Err(
                    "media_kind and media_width_mm must be supplied together when changing label stock."
                        .into(),
                );
            }
        };

        Ok(Self {
            name: value.name.map(LabelPrinterName::parse).transpose()?,
            host: value.host.map(LabelPrinterHost::parse).transpose()?,
            port: value.port.map(validate_port).transpose()?,
            model: value.model.map(validate_model).transpose()?,
            media,
            auto_cut: value.auto_cut,
        })
    }
}

#[derive(thiserror::Error)]
pub enum UpdateLabelPrinterError {
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

impl std::fmt::Debug for UpdateLabelPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateLabelPrinterError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateLabelPrinterError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateLabelPrinterError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateLabelPrinterError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateLabelPrinterError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateLabelPrinterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LabelPrinterDatabaseError> for UpdateLabelPrinterError {
    fn from(error: LabelPrinterDatabaseError) -> Self {
        match error {
            LabelPrinterDatabaseError::Validation(message) => Self::ValidationError(message),
            LabelPrinterDatabaseError::Conflict(message) => Self::ConflictError(message),
            LabelPrinterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Update a label printer",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_id=%printer_id)
)]
pub async fn update_label_printer(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    printer_id: LabelPrinterId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateLabelPrinterError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::LabelPrinter,
        Action::Update(*printer_id),
    )
    .await?
    {
        return Err(UpdateLabelPrinterError::Forbidden(
            "You are not allowed to update this label printer.".into(),
        ));
    }

    let update = UpdateLabelPrinter::try_from(payload.into_inner())
        .map_err(UpdateLabelPrinterError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_label_printer_for_update(&mut transaction, *printer_id)
        .await?
        .ok_or(UpdateLabelPrinterError::NotFound(
            "Label printer not found".into(),
        ))?;
    let printer =
        update_label_printer_in_database(&mut transaction, existing.printer_id, &update).await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Update,
        AuditResource::LabelPrinter,
        Some(printer.printer_id),
        update_label_printer_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update a label printer.")?;

    Ok(HttpResponse::Ok().json(LabelPrinterResponse::from(printer)))
}
