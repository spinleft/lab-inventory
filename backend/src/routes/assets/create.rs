use super::helpers::{
    default_pcs_unit_id, ensure_can_manage_threshold, map_database_error, normalize_asset_kind,
    normalize_tracking_mode, required_text, resolve_target_laboratory, validate_category,
    validate_default_unit, validate_minimum_stock_threshold,
};
use super::model::{AssetResponse, AssetRow};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    laboratory_id: Option<Uuid>,
    category_id: Option<Uuid>,
    asset_kind: String,
    tracking_mode: String,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Option<Uuid>,
    minimum_stock_quantity: Option<f64>,
    minimum_stock_unit_id: Option<Uuid>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Create an asset", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_asset(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id = resolve_target_laboratory(&actor, payload.laboratory_id)?;
    let asset_kind = normalize_asset_kind(&payload.asset_kind)?;
    let tracking_mode = normalize_tracking_mode(&payload.tracking_mode)?;
    let name = required_text(&payload.name, "name")?;
    validate_category(pool.get_ref(), laboratory_id, payload.category_id).await?;
    let default_unit_id = match payload.default_unit_id {
        Some(unit_id) => unit_id,
        None => default_pcs_unit_id(pool.get_ref()).await?,
    };
    validate_default_unit(pool.get_ref(), tracking_mode, default_unit_id).await?;
    validate_minimum_stock_threshold(
        pool.get_ref(),
        tracking_mode,
        default_unit_id,
        payload.minimum_stock_quantity,
        payload.minimum_stock_unit_id,
    )
    .await?;
    if payload.minimum_stock_quantity.is_some() || payload.minimum_stock_unit_id.is_some() {
        ensure_can_manage_threshold(&actor, laboratory_id)?;
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let asset = sqlx::query_as::<_, AssetRow>(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            category_id,
            asset_kind,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            minimum_stock_quantity,
            minimum_stock_unit_id,
            public_notes,
            internal_notes
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING
            asset_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = $2) AS laboratory_name,
            category_id,
            (SELECT name FROM asset_categories WHERE category_id = $3) AS category_name,
            asset_kind,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            (SELECT code FROM units WHERE unit_id = $9) AS default_unit_code,
            minimum_stock_quantity,
            minimum_stock_unit_id,
            (SELECT code FROM units WHERE unit_id = minimum_stock_unit_id) AS minimum_stock_unit_code,
            public_notes,
            internal_notes,
            is_archived,
            created_at,
            updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(payload.category_id)
    .bind(asset_kind)
    .bind(tracking_mode)
    .bind(name)
    .bind(payload.model.as_deref())
    .bind(payload.manufacturer.as_deref())
    .bind(default_unit_id)
    .bind(payload.minimum_stock_quantity)
    .bind(payload.minimum_stock_unit_id)
    .bind(payload.public_notes.as_deref())
    .bind(payload.internal_notes.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(asset.laboratory_id),
        AuditAction::Create,
        AuditResource::Asset,
        Some(asset.asset_id),
        json!({ "name": asset.name, "tracking_mode": asset.tracking_mode }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(AssetResponse::from_row(asset, &actor)))
}
