//! What this deployment calls itself, for clients that need to mint links back
//! to it.
//!
//! QR codes on printed labels embed the node id so that a scan can be resolved
//! by *any* instance: the scanner compares the id against its own, then against
//! its federation trusts, and follows whichever matches. The client builds
//! those payloads itself, so it needs the two values here and nothing else.
use super::queries::fetch_local_node_id;
use crate::configuration::PublicWebUrl;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
struct InstanceIdentityResponse {
    node_id: Uuid,
    public_web_url: String,
}

#[derive(thiserror::Error)]
pub enum InstanceIdentityError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for InstanceIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for InstanceIdentityError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[tracing::instrument(name = "Get instance identity", skip(pool, public_web_url))]
pub async fn get_instance_identity(
    pool: web::Data<PgPool>,
    public_web_url: web::Data<PublicWebUrl>,
) -> Result<HttpResponse, InstanceIdentityError> {
    // The migration mints this row, and startup refuses to run without it, so
    // its absence is a broken deployment rather than a client error.
    let node_id = fetch_local_node_id(&pool)
        .await?
        .ok_or_else(|| anyhow!("This server has no federation node identity"))?;

    Ok(HttpResponse::Ok().json(InstanceIdentityResponse {
        node_id,
        public_web_url: public_web_url.0.clone(),
    }))
}
