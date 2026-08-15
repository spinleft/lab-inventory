use super::model::LabelPrinterResponse;
use super::queries::fetch_label_printer;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::LabelPrinterId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetLabelPrinterError {
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetLabelPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetLabelPrinterError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetLabelPrinterError::NotFound(_) => StatusCode::NOT_FOUND,
            GetLabelPrinterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a label printer",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, printer_id=%printer_id)
)]
pub async fn get_label_printer(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    printer_id: LabelPrinterId,
) -> Result<HttpResponse, GetLabelPrinterError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::LabelPrinter,
        Action::Read(*printer_id),
    )
    .await?
    {
        return Err(GetLabelPrinterError::NotFound(
            "Label printer not found".into(),
        ));
    }

    let printer =
        fetch_label_printer(&pool, *printer_id)
            .await?
            .ok_or(GetLabelPrinterError::NotFound(
                "Label printer not found".into(),
            ))?;

    Ok(HttpResponse::Ok().json(LabelPrinterResponse::from(printer)))
}
