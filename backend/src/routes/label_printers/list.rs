use super::model::LabelPrinterResponse;
use super::queries::fetch_label_printers;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum ListLabelPrintersError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListLabelPrintersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListLabelPrintersError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListLabelPrintersError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListLabelPrintersError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List label printers",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id)
)]
pub async fn list_label_printers(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, ListLabelPrintersError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::LabelPrinter,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListLabelPrintersError::Forbidden(
            "You don't have permission to view label printers.".into(),
        ));
    }

    let printers: Vec<_> = fetch_label_printers(&pool, laboratory_id)
        .await?
        .into_iter()
        .map(LabelPrinterResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(printers))
}
