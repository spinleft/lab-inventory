use super::model::{LocationResponse, LocationRow, fetch_location};
use crate::access_control::{Actor, get_actor};
use crate::domain::{LaboratoryId, LocationId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

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
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_locations(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ListLocationsError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(|e| ListLocationsError::UnexpectedError(anyhow!("{e}")))?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListLocationsError::UnexpectedError)?
        .ok_or(ListLocationsError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_read_permission(&actor, &laboratory_id)?;

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

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), ListLocationsError> {
    if actor.can_read_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(ListLocationsError::Forbidden(
            "You do not have permission to view this location".into(),
        ))
    }
}

async fn fetch_locations(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    root_path: Option<&str>,
) -> Result<Vec<LocationRow>, ListLocationsError> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListLocationsError::UnexpectedError(e.into()))
}
