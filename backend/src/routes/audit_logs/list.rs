use super::model::AuditLog;
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AuditLogListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub actor_user_id: Option<Uuid>,
    pub actor_laboratory_id: Option<Uuid>,
    pub target_laboratory_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub created_from: Option<DateTime<Utc>>,
    pub created_to: Option<DateTime<Utc>>,
}

#[tracing::instrument(name = "List audit logs", skip(pool), fields(user_id=%user_id))]
pub async fn list_audit_logs(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AuditLogListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if !actor.is_system_admin() && !actor.is_lab_admin() {
        return Err(ApiError::Forbidden);
    }
    let total = fetch_audit_log_count(pool.get_ref(), &actor, &query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(audit_log_select());
    push_audit_log_filters(&mut builder, &actor, &query);
    builder.push(" ORDER BY audit_logs.created_at DESC");
    builder.push(" LIMIT ");
    builder.push_bind(query.pagination.limit()?);
    builder.push(" OFFSET ");
    builder.push_bind(query.pagination.offset()?);
    let audit_logs = builder
        .build_query_as::<AuditLog>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        audit_logs,
        &query.pagination,
        total,
    )?))
}

async fn fetch_audit_log_count(
    pool: &PgPool,
    actor: &Actor,
    query: &AuditLogListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
        LEFT JOIN laboratories AS actor_laboratory ON actor_laboratory.laboratory_id = audit_logs.actor_laboratory_id
        LEFT JOIN laboratories AS target_laboratory ON target_laboratory.laboratory_id = audit_logs.target_laboratory_id
        "#,
    );
    push_audit_log_filters(&mut builder, actor, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn audit_log_select() -> &'static str {
    r#"
    SELECT
        audit_logs.audit_log_id,
        audit_logs.actor_user_id,
        actor_user.username AS actor_username,
        audit_logs.actor_laboratory_id,
        actor_laboratory.name AS actor_laboratory_name,
        audit_logs.target_laboratory_id,
        target_laboratory.name AS target_laboratory_name,
        audit_logs.action,
        audit_logs.resource_type,
        audit_logs.resource_id,
        audit_logs.details,
        audit_logs.created_at
    FROM audit_logs
    LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
    LEFT JOIN laboratories AS actor_laboratory ON actor_laboratory.laboratory_id = audit_logs.actor_laboratory_id
    LEFT JOIN laboratories AS target_laboratory ON target_laboratory.laboratory_id = audit_logs.target_laboratory_id
    "#
}

fn push_audit_log_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    query: &AuditLogListQuery,
) {
    builder.push(" WHERE TRUE");
    if !actor.is_system_admin() {
        if let Some(laboratory_id) = actor.laboratory_id {
            builder.push(" AND (audit_logs.actor_laboratory_id = ");
            builder.push_bind(laboratory_id);
            builder.push(" OR audit_logs.target_laboratory_id = ");
            builder.push_bind(laboratory_id);
            builder.push(")");
        } else {
            builder.push(" AND FALSE");
        }
    }
    if let Some(actor_user_id) = query.actor_user_id {
        builder.push(" AND audit_logs.actor_user_id = ");
        builder.push_bind(actor_user_id);
    }
    if let Some(actor_laboratory_id) = query.actor_laboratory_id {
        builder.push(" AND audit_logs.actor_laboratory_id = ");
        builder.push_bind(actor_laboratory_id);
    }
    if let Some(target_laboratory_id) = query.target_laboratory_id {
        builder.push(" AND audit_logs.target_laboratory_id = ");
        builder.push_bind(target_laboratory_id);
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
