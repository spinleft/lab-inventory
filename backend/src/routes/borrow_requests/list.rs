use super::model::{
    BorrowRequestError, BorrowRequestResponse, actor_for_user, borrow_request_inventory_select,
    validate_borrow_request_status, validate_resolver_actor,
};
use crate::domain::{LaboratoryId, UserId};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListBorrowRequestsQuery {
    pub status: Option<String>,
}

#[tracing::instrument(
    name = "List borrow requests",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_borrow_requests(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListBorrowRequestsQuery>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(|e| BorrowRequestError::UnexpectedError(anyhow::anyhow!(e)))?;
    validate_resolver_actor(&actor, laboratory_id)?;
    let status = validate_borrow_request_status(query.status.clone())?;

    let mut builder = QueryBuilder::<Postgres>::new(borrow_request_inventory_select());
    builder.push(
        " INNER JOIN federation_borrow_requests AS requests ON requests.inventory_item_id = asset_inventory_items.inventory_item_id WHERE requests.local_laboratory_id = ",
    );
    builder.push_bind(*laboratory_id);
    if let Some(status) = status {
        builder.push(" AND requests.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY requests.created_at DESC, requests.borrow_request_id DESC");

    let rows = builder
        .build_query_as::<super::model::BorrowRequestRow>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(BorrowRequestResponse::from)
            .collect::<Vec<_>>(),
    ))
}
