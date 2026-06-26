use super::error::AttachmentError;
use super::model::{
    AttachmentResponse, actor_for_user, attachment_audit_json, attachment_columns,
    fetch_attachment_for_update, validate_write_permission,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AttachmentDescription, AttachmentDisplayName, AttachmentId, AttachmentVisibility, LaboratoryId,
    NullableUpdate, UpdateAttachment, UserId,
};
use actix_web::{HttpResponse, web};
use anyhow::{Context, anyhow};
use serde_json::{Value, json};
use sqlx::PgPool;

#[tracing::instrument(
    name = "Update attachment metadata",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn update_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    attachment_id: web::Path<AttachmentId>,
    payload: web::Json<Value>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let patch = parse_attachment_patch(payload.into_inner())?;
    let description: Option<&str> = match &patch.description {
        NullableUpdate::Set(value) => Some(value.as_ref()),
        NullableUpdate::Unchanged | NullableUpdate::Clear => None,
    };
    let attachment_id = attachment_id.into_inner();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let before = fetch_attachment_for_update(&mut transaction, *attachment_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Attachment not found".into()))?;
    let laboratory_id = LaboratoryId::parse(before.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    validate_write_permission(&actor, &laboratory_id)?;

    let after = sqlx::query_as::<_, super::model::AttachmentRow>(&format!(
        r#"
            UPDATE attachments
            SET
                display_name = COALESCE($2, display_name),
                description = CASE WHEN $3 THEN $4 ELSE description END,
                visibility = COALESCE($5, visibility),
                updated_at = now()
            WHERE attachment_id = $1
              AND deleted_at IS NULL
            RETURNING
                {}
            "#,
        attachment_columns()
    ))
    .bind(before.attachment_id)
    .bind(patch.display_name.as_ref().map(|value| value.as_ref()))
    .bind(patch.description.is_changed())
    .bind(description)
    .bind(patch.visibility.as_ref().map(AttachmentVisibility::as_str))
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::Attachment,
        Some(after.attachment_id),
        json!({
            "before": attachment_audit_json(&before),
            "after": attachment_audit_json(&after),
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update attachment")?;

    Ok(HttpResponse::Ok().json(AttachmentResponse::from(after)))
}

fn parse_attachment_patch(value: Value) -> Result<UpdateAttachment, AttachmentError> {
    let object = value.as_object().ok_or_else(|| {
        AttachmentError::ValidationError("Attachment patch body must be a JSON object".into())
    })?;
    for key in object.keys() {
        if !matches!(key.as_str(), "display_name" | "description" | "visibility") {
            return Err(AttachmentError::ValidationError(format!(
                "Unsupported attachment patch field: {key}"
            )));
        }
    }

    let display_name = match object.get("display_name") {
        Some(Value::String(value)) => Some(
            AttachmentDisplayName::parse(value.clone())
                .map_err(AttachmentError::ValidationError)?,
        ),
        Some(_) => {
            return Err(AttachmentError::ValidationError(
                "display_name must be a string".into(),
            ));
        }
        None => None,
    };
    let description = match object.get("description") {
        Some(Value::Null) => NullableUpdate::Clear,
        Some(Value::String(value)) => {
            match AttachmentDescription::parse_optional(value.clone())
                .map_err(AttachmentError::ValidationError)?
            {
                Some(description) => NullableUpdate::Set(description),
                None => NullableUpdate::Clear,
            }
        }
        Some(_) => {
            return Err(AttachmentError::ValidationError(
                "description must be a string or null".into(),
            ));
        }
        None => NullableUpdate::Unchanged,
    };
    let visibility = match object.get("visibility") {
        Some(Value::String(value)) => {
            Some(AttachmentVisibility::parse(value).map_err(AttachmentError::ValidationError)?)
        }
        Some(_) => {
            return Err(AttachmentError::ValidationError(
                "visibility must be a string".into(),
            ));
        }
        None => None,
    };

    Ok(UpdateAttachment::new(display_name, description, visibility))
}
