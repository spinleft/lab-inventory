use super::error::AttachmentError;
use super::model::{
    AttachmentClaimInput, AttachmentResponse, AttachmentRow, AttachmentTarget, AttachmentUploadRow,
    actor_for_user, attachment_audit_json, attachment_columns, fetch_asset_target_for_update,
    fetch_inventory_item_target_for_update, map_database_error, validate_write_permission,
};
use crate::access_control::Actor;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AttachmentClaim, AttachmentUploadId, LaboratoryId, UserId};
use actix_web::{HttpResponse, web};
use anyhow::{Context, anyhow};
use chrono::Utc;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[tracing::instrument(
    name = "Create an asset attachment",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn create_asset_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
    payload: web::Json<AttachmentClaimInput>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset_id = asset_id.into_inner();
    let target = fetch_asset_target_for_update(&mut transaction, asset_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Asset not found".into()))?;
    let laboratory_id = LaboratoryId::parse(target.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    validate_write_permission(&actor, &laboratory_id)?;

    let claim = AttachmentClaim::try_from(payload.into_inner())
        .map_err(AttachmentError::ValidationError)?;
    let mut rows = claim_asset_attachments(
        &mut transaction,
        &actor,
        laboratory_id,
        asset_id,
        std::slice::from_ref(&claim),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create attachment")?;

    Ok(HttpResponse::Created().json(AttachmentResponse::from(rows.remove(0))))
}

#[tracing::instrument(
    name = "Create an inventory item attachment",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn create_inventory_item_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<AttachmentClaimInput>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let inventory_item_id = inventory_item_id.into_inner();
    let target = fetch_inventory_item_target_for_update(&mut transaction, inventory_item_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Inventory item not found".into()))?;
    let laboratory_id = LaboratoryId::parse(target.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    validate_write_permission(&actor, &laboratory_id)?;

    let claim = AttachmentClaim::try_from(payload.into_inner())
        .map_err(AttachmentError::ValidationError)?;
    let mut rows = claim_inventory_item_attachments(
        &mut transaction,
        &actor,
        laboratory_id,
        inventory_item_id,
        std::slice::from_ref(&claim),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create attachment")?;

    Ok(HttpResponse::Created().json(AttachmentResponse::from(rows.remove(0))))
}

pub(crate) async fn claim_asset_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    claims: &[AttachmentClaim],
) -> Result<Vec<AttachmentRow>, AttachmentError> {
    claim_uploaded_attachments(
        transaction,
        actor,
        laboratory_id,
        AttachmentTarget::Asset(asset_id),
        claims,
    )
    .await
}

pub(crate) async fn claim_inventory_item_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    laboratory_id: LaboratoryId,
    inventory_item_id: Uuid,
    claims: &[AttachmentClaim],
) -> Result<Vec<AttachmentRow>, AttachmentError> {
    claim_uploaded_attachments(
        transaction,
        actor,
        laboratory_id,
        AttachmentTarget::InventoryItem(inventory_item_id),
        claims,
    )
    .await
}

async fn claim_uploaded_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    laboratory_id: LaboratoryId,
    target: AttachmentTarget,
    claims: &[AttachmentClaim],
) -> Result<Vec<AttachmentRow>, AttachmentError> {
    if claims.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = HashSet::new();
    for claim in claims {
        if !seen.insert(*claim.upload_id) {
            return Err(AttachmentError::ValidationError(
                "attachments cannot contain duplicate upload_id values".into(),
            ));
        }
    }

    let mut rows = Vec::with_capacity(claims.len());
    for claim in claims {
        let upload = fetch_upload_for_update(transaction, claim.upload_id).await?;
        validate_upload_claim(actor, laboratory_id, &upload)?;
        let display_name = match claim.display_name.clone() {
            Some(value) => value.as_ref().to_string(),
            None => upload.original_file_name.clone(),
        };
        let description = claim.description.as_ref().map(|value| value.as_ref());
        let visibility = claim.visibility.as_str();
        let (asset_id, inventory_item_id) = match target {
            AttachmentTarget::Asset(asset_id) => (Some(asset_id), None),
            AttachmentTarget::InventoryItem(inventory_item_id) => (None, Some(inventory_item_id)),
        };
        let row = sqlx::query_as::<_, AttachmentRow>(&format!(
            r#"
                INSERT INTO attachments (
                    attachment_id,
                    laboratory_id,
                    asset_id,
                    inventory_item_id,
                    display_name,
                    original_file_name,
                    description,
                    mime_type,
                    file_size_bytes,
                    sha256_hex,
                    storage_backend,
                    storage_key,
                    visibility,
                    uploaded_by_user_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING
                    {}
                "#,
            attachment_columns()
        ))
        .bind(Uuid::new_v4())
        .bind(*laboratory_id)
        .bind(asset_id)
        .bind(inventory_item_id)
        .bind(&display_name)
        .bind(&upload.original_file_name)
        .bind(description)
        .bind(upload.mime_type.as_deref())
        .bind(upload.file_size_bytes)
        .bind(&upload.sha256_hex)
        .bind(&upload.storage_backend)
        .bind(&upload.storage_key)
        .bind(visibility)
        .bind(upload.uploaded_by_user_id)
        .fetch_one(transaction.as_mut())
        .await
        .map_err(map_database_error)?;

        sqlx::query(
            r#"
            UPDATE attachment_uploads
            SET consumed_at = now()
            WHERE upload_id = $1
            "#,
        )
        .bind(upload.upload_id)
        .execute(transaction.as_mut())
        .await
        .map_err(|e| AttachmentError::UnexpectedError(e.into()))?;

        record_audit(
            transaction,
            actor,
            AuditAction::Create,
            AuditResource::Attachment,
            Some(row.attachment_id),
            json!({
                "created": attachment_audit_json(&row),
            }),
        )
        .await?;
        rows.push(row);
    }
    Ok(rows)
}

async fn fetch_upload_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: AttachmentUploadId,
) -> Result<AttachmentUploadRow, AttachmentError> {
    sqlx::query_as::<_, AttachmentUploadRow>(
        r#"
        SELECT
            upload_id,
            laboratory_id,
            storage_backend,
            storage_key,
            original_file_name,
            mime_type,
            file_size_bytes,
            sha256_hex,
            uploaded_by_user_id,
            expires_at,
            consumed_at
        FROM attachment_uploads
        WHERE upload_id = $1
        FOR UPDATE
        "#,
    )
    .bind(*upload_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))?
    .ok_or_else(|| AttachmentError::ValidationError("Attachment upload not found".into()))
}

fn validate_upload_claim(
    actor: &Actor,
    laboratory_id: LaboratoryId,
    upload: &AttachmentUploadRow,
) -> Result<(), AttachmentError> {
    if upload.laboratory_id != *laboratory_id {
        return Err(AttachmentError::ValidationError(
            "Attachment upload does not belong to this laboratory".into(),
        ));
    }
    if upload.uploaded_by_user_id != Some(*actor.user_id) {
        return Err(AttachmentError::Forbidden(
            "You can only consume attachment uploads created by your own user".into(),
        ));
    }
    if upload.consumed_at.is_some() {
        return Err(AttachmentError::ValidationError(
            "Attachment upload has already been consumed".into(),
        ));
    }
    if upload.expires_at <= Utc::now() {
        return Err(AttachmentError::ValidationError(
            "Attachment upload has expired".into(),
        ));
    }
    Ok(())
}
