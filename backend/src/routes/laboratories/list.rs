use super::model::{LaboratoryResponse, LaboratoryRow};
use crate::access_control::get_actor;
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListLaboratoriesError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListLaboratoriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListLaboratoriesError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListLaboratoriesError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListLaboratoriesError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "List laboratories", skip(pool), fields(actor_user_id=%actor_user_id))]
pub async fn list_laboratories(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ListLaboratoriesError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListLaboratoriesError::UnexpectedError)?
        .ok_or(ListLaboratoriesError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    if !actor.is_admin() {
        return Err(ListLaboratoriesError::Forbidden(
            "You don't have permission to list laboratories.".into(),
        ));
    }

    let actor_laboratory_id = if actor.is_lab_admin() {
        let Some(laboratory_id) = actor.laboratory_id.map(Uuid::from) else {
            return Ok(HttpResponse::Ok().json(Vec::<LaboratoryResponse>::new()));
        };
        Some(laboratory_id)
    } else {
        None
    };
    let laboratories: Vec<_> = fetch_laboratories(&pool, actor_laboratory_id)
        .await?
        .into_iter()
        .map(LaboratoryResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(laboratories))
}

async fn fetch_laboratories(
    pool: &PgPool,
    laboratory_id: Option<Uuid>,
) -> Result<Vec<LaboratoryRow>, ListLaboratoriesError> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE $1::uuid IS NULL OR laboratory_id = $1
        ORDER BY name
        "#,
        laboratory_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListLaboratoriesError::UnexpectedError(e.into()))
}
