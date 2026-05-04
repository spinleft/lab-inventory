use super::helpers::attachment_select;
use super::model::Attachment;
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AttachmentListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub visibility: Option<String>,
}

#[tracing::instrument(name = "List attachment metadata", skip(pool), fields(user_id=%user_id))]
pub async fn list_attachments(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AttachmentListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (attachments, total) = fetch_attachments(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        attachments,
        &query.pagination,
        total,
    )?))
}

pub(crate) async fn fetch_attachments(
    pool: &PgPool,
    actor: &Actor,
    query: &AttachmentListQuery,
    paginate: bool,
) -> Result<(Vec<Attachment>, i64), ApiError> {
    let total = fetch_attachment_count(pool, actor, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(attachment_select());
    push_attachment_filters(&mut builder, actor, query);
    builder.push(" ORDER BY attachments.created_at DESC");
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let attachments = builder
        .build_query_as::<Attachment>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok((attachments, total))
}

async fn fetch_attachment_count(
    pool: &PgPool,
    actor: &Actor,
    query: &AttachmentListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM attachments
        INNER JOIN laboratories USING (laboratory_id)
        LEFT JOIN users ON users.user_id = attachments.uploaded_by_user_id
        "#,
    );
    push_attachment_filters(&mut builder, actor, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_attachment_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    query: &AttachmentListQuery,
) {
    builder.push(" WHERE TRUE");
    if !actor.is_owner() {
        builder.push(" AND (attachments.visibility = 'public'");
        if let Some(laboratory_id) = actor.laboratory_id {
            builder.push(" OR attachments.laboratory_id = ");
            builder.push_bind(laboratory_id);
        }
        builder.push(")");
    }
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (attachments.file_name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(attachments.mime_type, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND attachments.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }
    if let Some(resource_type) = query
        .resource_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND attachments.resource_type = ");
        builder.push_bind(resource_type.to_owned());
    }
    if let Some(resource_id) = query.resource_id {
        builder.push(" AND attachments.resource_id = ");
        builder.push_bind(resource_id);
    }
    if let Some(visibility) = query
        .visibility
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND attachments.visibility = ");
        builder.push_bind(visibility.to_owned());
    }
}
