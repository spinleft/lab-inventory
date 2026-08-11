use super::model::BorrowRequestResponse;
use super::queries::fetch_borrow_requests;
use super::service::{BorrowRequestError, validate_borrow_request_status, validate_resolver_actor};
use crate::access_control::LaboratoryContext;
use crate::domain::UserId;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListBorrowRequestsQuery {
    pub status: Option<String>,
}

#[tracing::instrument(
    name = "List borrow requests",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_borrow_requests(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    query: web::Query<ListBorrowRequestsQuery>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    validate_resolver_actor(actor, laboratory_id)?;
    let status = validate_borrow_request_status(query.status.clone())?;

    let requests = fetch_borrow_requests(&pool, laboratory_id, status).await?;

    Ok(HttpResponse::Ok().json(
        requests
            .into_iter()
            .map(BorrowRequestResponse::from)
            .collect::<Vec<_>>(),
    ))
}
