use super::model::LocationResponse;
use super::queries::{fetch_location, fetch_locations};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::LocationId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct ListQuery {
    root_location_id: Option<LocationId>,
}

#[derive(thiserror::Error)]
pub enum ListLocationsError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListLocationsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListLocationsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListLocationsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListLocationsError::NotFound(_) => StatusCode::NOT_FOUND,
            ListLocationsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List locations",
    skip(pool, query),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_locations(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ListLocationsError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::Location,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListLocationsError::Forbidden(
            "You don't have permission to view locations.".into(),
        ));
    }

    let root_path = match query.root_location_id {
        Some(root_location_id) => {
            let root = fetch_location(&pool, root_location_id).await?.ok_or(
                ListLocationsError::NotFound("Root location not found".into()),
            )?;
            if root.laboratory_id != *laboratory_id {
                return Err(ListLocationsError::NotFound(
                    "Root location not found".into(),
                ));
            }
            Some(root.path)
        }
        None => None,
    };

    let locations: Vec<_> = fetch_locations(&pool, laboratory_id, root_path.as_deref())
        .await?
        .into_iter()
        .map(LocationResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(locations))
}
