use super::error::AttachmentError;
use super::model::{
    AttachmentResponse, actor_for_user, fetch_attachment, validate_attachment_read_permission,
};
use crate::domain::{AttachmentId, UserId};
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(
    name = "Get attachment metadata",
    skip(pool),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn get_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    attachment_id: web::Path<AttachmentId>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let attachment_id = attachment_id.into_inner();
    let row = fetch_attachment(&pool, *attachment_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Attachment not found".into()))?;
    validate_attachment_read_permission(&actor, &row)?;
    Ok(HttpResponse::Ok().json(AttachmentResponse::from(row)))
}
