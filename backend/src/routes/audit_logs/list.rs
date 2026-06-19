use super::model::AuditLog;
use crate::access_control::get_actor;
use crate::domain::UserId;
use crate::routes::{PaginatedResponse, Pagination, PaginationError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AuditLogListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub actor_user_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub created_from: Option<DateTime<Utc>>,
    pub created_to: Option<DateTime<Utc>>,
}

#[derive(thiserror::Error)]
pub enum ListAuditLogsError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListAuditLogsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAuditLogsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAuditLogsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ListAuditLogsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAuditLogsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PaginationError> for ListAuditLogsError {
    fn from(error: PaginationError) -> Self {
        Self::ValidationError(error.to_string())
    }
}

#[tracing::instrument(name = "List audit logs", skip(pool), fields(actor_user_id=%actor_user_id))]
pub async fn list_audit_logs(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AuditLogListQuery>,
) -> Result<HttpResponse, ListAuditLogsError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListAuditLogsError::UnexpectedError)?
        .ok_or(ListAuditLogsError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    if !(actor.is_root() || actor.is_super_admin()) {
        return Err(ListAuditLogsError::Forbidden(
            "You don't have permission to list audit logs.".into(),
        ));
    }

    let total = fetch_audit_log_count(&pool, &query).await?;
    let limit = query.pagination.limit()?;
    let offset = query.pagination.offset()?;

    let mut builder = QueryBuilder::<Postgres>::new(audit_log_select());
    push_audit_log_filters(&mut builder, &query);
    builder.push(" ORDER BY audit_logs.created_at DESC");
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    let audit_logs = builder
        .build_query_as::<AuditLog>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| ListAuditLogsError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        audit_logs,
        &query.pagination,
        total,
    )?))
}

async fn fetch_audit_log_count(
    pool: &PgPool,
    query: &AuditLogListQuery,
) -> Result<i64, ListAuditLogsError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
        "#,
    );
    push_audit_log_filters(&mut builder, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ListAuditLogsError::UnexpectedError(e.into()))
}

fn audit_log_select() -> &'static str {
    r#"
    SELECT
        audit_logs.audit_log_id,
        audit_logs.actor_user_id,
        actor_user.username AS actor_username,
        audit_logs.action,
        audit_logs.resource_type,
        audit_logs.resource_id,
        audit_logs.details,
        audit_logs.created_at
    FROM audit_logs
    LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
    "#
}

fn push_audit_log_filters(builder: &mut QueryBuilder<'_, Postgres>, query: &AuditLogListQuery) {
    builder.push(" WHERE TRUE");
    if let Some(actor_user_id) = query.actor_user_id {
        builder.push(" AND audit_logs.actor_user_id = ");
        builder.push_bind(actor_user_id);
    }
    if let Some(action) = query
        .action
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND audit_logs.action = ");
        builder.push_bind(action.to_owned());
    }
    if let Some(resource_type) = query
        .resource_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND audit_logs.resource_type = ");
        builder.push_bind(resource_type.to_owned());
    }
    if let Some(resource_id) = query.resource_id {
        builder.push(" AND audit_logs.resource_id = ");
        builder.push_bind(resource_id);
    }
    if let Some(created_from) = query.created_from {
        builder.push(" AND audit_logs.created_at >= ");
        builder.push_bind(created_from);
    }
    if let Some(created_to) = query.created_to {
        builder.push(" AND audit_logs.created_at <= ");
        builder.push_bind(created_to);
    }
}
