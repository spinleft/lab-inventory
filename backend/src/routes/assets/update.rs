use super::helpers::{
    ensure_can_manage_threshold, ensure_can_write, fetch_asset, map_database_error,
    normalize_asset_kind, normalize_tracking_mode, required_text, validate_category,
    validate_default_unit, validate_minimum_stock_threshold,
};
use super::model::{AssetResponse, AssetRow};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    category_id: Option<Uuid>,
    asset_kind: Option<String>,
    tracking_mode: Option<String>,
    name: Option<String>,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Option<Uuid>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    is_archived: Option<bool>,
    #[serde(flatten)]
    extra_fields: BTreeMap<String, Value>,
}

enum MinimumStockChange {
    Keep,
    Clear,
    Set { quantity: f64, unit_id: Uuid },
}

impl MinimumStockChange {
    fn should_update(&self) -> bool {
        !matches!(self, Self::Keep)
    }

    fn quantity(&self, existing: Option<f64>) -> Option<f64> {
        match self {
            Self::Keep => existing,
            Self::Clear => None,
            Self::Set { quantity, .. } => Some(*quantity),
        }
    }

    fn unit_id(&self, existing: Option<Uuid>) -> Option<Uuid> {
        match self {
            Self::Keep => existing,
            Self::Clear => None,
            Self::Set { unit_id, .. } => Some(*unit_id),
        }
    }
}

fn parse_minimum_stock_change(payload: &JsonData) -> Result<MinimumStockChange, ApiError> {
    let quantity = payload.extra_fields.get("minimum_stock_quantity");
    let unit_id = payload.extra_fields.get("minimum_stock_unit_id");
    match (quantity, unit_id) {
        (None, None) => Ok(MinimumStockChange::Keep),
        (Some(Value::Null), Some(Value::Null)) => Ok(MinimumStockChange::Clear),
        (Some(quantity), Some(unit_id)) if !quantity.is_null() && !unit_id.is_null() => {
            let quantity = serde_json::from_value::<f64>(quantity.clone()).map_err(|_| {
                ApiError::BadRequest("minimum_stock_quantity must be a number".into())
            })?;
            let unit_id = serde_json::from_value::<Uuid>(unit_id.clone())
                .map_err(|_| ApiError::BadRequest("minimum_stock_unit_id must be a UUID".into()))?;
            Ok(MinimumStockChange::Set { quantity, unit_id })
        }
        _ => Err(ApiError::BadRequest(
            "minimum_stock_quantity and minimum_stock_unit_id must be provided together".into(),
        )),
    }
}

#[tracing::instrument(name = "Update an asset", skip(pool, payload), fields(user_id=%user_id, asset_id=%asset_id))]
pub async fn update_asset(
    user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let asset_id = asset_id.into_inner();
    let existing = fetch_asset(pool.get_ref(), asset_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;

    validate_category(pool.get_ref(), existing.laboratory_id, payload.category_id).await?;
    let asset_kind = match payload.asset_kind.as_deref() {
        Some(asset_kind) => Some(normalize_asset_kind(asset_kind)?),
        None => None,
    };
    let tracking_mode = match payload.tracking_mode.as_deref() {
        Some(tracking_mode) => {
            let tracking_mode = normalize_tracking_mode(tracking_mode)?;
            let inventory_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM asset_inventory_items WHERE asset_id = $1",
            )
            .bind(asset_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
            if inventory_count > 0 && tracking_mode != existing.tracking_mode {
                return Err(ApiError::Conflict(
                    "Cannot change tracking_mode after inventory items exist".into(),
                ));
            }
            Some(tracking_mode)
        }
        None => None,
    };
    let effective_tracking_mode = tracking_mode.unwrap_or(existing.tracking_mode.as_str());
    let default_unit_id = payload.default_unit_id.unwrap_or(existing.default_unit_id);
    validate_default_unit(pool.get_ref(), effective_tracking_mode, default_unit_id).await?;
    let minimum_stock_change = parse_minimum_stock_change(&payload)?;
    if minimum_stock_change.should_update() {
        ensure_can_manage_threshold(&actor, existing.laboratory_id)?;
    }
    let effective_minimum_stock_quantity =
        minimum_stock_change.quantity(existing.minimum_stock_quantity);
    let effective_minimum_stock_unit_id =
        minimum_stock_change.unit_id(existing.minimum_stock_unit_id);
    validate_minimum_stock_threshold(
        pool.get_ref(),
        effective_tracking_mode,
        default_unit_id,
        effective_minimum_stock_quantity,
        effective_minimum_stock_unit_id,
    )
    .await?;
    let name = match payload.name.as_deref() {
        Some(name) => Some(required_text(name, "name")?),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let asset = sqlx::query_as::<_, AssetRow>(
        r#"
        UPDATE assets
        SET
            category_id = COALESCE($2, category_id),
            asset_kind = COALESCE($3, asset_kind),
            tracking_mode = COALESCE($4, tracking_mode),
            name = COALESCE($5, name),
            model = COALESCE($6, model),
            manufacturer = COALESCE($7, manufacturer),
            default_unit_id = COALESCE($8, default_unit_id),
            public_notes = COALESCE($9, public_notes),
            internal_notes = COALESCE($10, internal_notes),
            is_archived = COALESCE($11, is_archived),
            minimum_stock_quantity = CASE WHEN $12 THEN $13 ELSE minimum_stock_quantity END,
            minimum_stock_unit_id = CASE WHEN $12 THEN $14 ELSE minimum_stock_unit_id END,
            updated_at = now()
        WHERE asset_id = $1
        RETURNING
            asset_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = assets.laboratory_id) AS laboratory_name,
            category_id,
            (SELECT name FROM asset_categories WHERE category_id = assets.category_id) AS category_name,
            asset_kind,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            (SELECT code FROM units WHERE unit_id = assets.default_unit_id) AS default_unit_code,
            minimum_stock_quantity,
            minimum_stock_unit_id,
            (SELECT code FROM units WHERE unit_id = assets.minimum_stock_unit_id) AS minimum_stock_unit_code,
            public_notes,
            internal_notes,
            is_archived,
            created_at,
            updated_at
        "#,
    )
    .bind(asset_id)
    .bind(payload.category_id)
    .bind(asset_kind)
    .bind(tracking_mode)
    .bind(name)
    .bind(payload.model.as_deref())
    .bind(payload.manufacturer.as_deref())
    .bind(payload.default_unit_id)
    .bind(payload.public_notes.as_deref())
    .bind(payload.internal_notes.as_deref())
    .bind(payload.is_archived)
    .bind(minimum_stock_change.should_update())
    .bind(minimum_stock_change.quantity(None))
    .bind(minimum_stock_change.unit_id(None))
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(asset.laboratory_id),
        AuditAction::Update,
        AuditResource::Asset,
        Some(asset.asset_id),
        json!({ "name": asset.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(AssetResponse::from_row(asset, &actor)))
}
