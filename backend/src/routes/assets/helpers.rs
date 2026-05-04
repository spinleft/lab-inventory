use super::model::AssetRow;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn fetch_asset(pool: &PgPool, asset_id: Uuid) -> Result<AssetRow, ApiError> {
    sqlx::query_as::<_, AssetRow>(ASSET_SELECT)
        .bind(asset_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

const ASSET_SELECT: &str = r#"
SELECT
    assets.asset_id,
    assets.laboratory_id,
    laboratories.name AS laboratory_name,
    assets.category_id,
    asset_categories.name AS category_name,
    assets.asset_kind,
    assets.tracking_mode,
    assets.name,
    assets.model,
    assets.manufacturer,
    assets.default_unit_id,
    units.code AS default_unit_code,
    assets.minimum_stock_quantity,
    assets.minimum_stock_unit_id,
    minimum_stock_units.code AS minimum_stock_unit_code,
    assets.public_notes,
    assets.internal_notes,
    assets.is_archived,
    assets.created_at,
    assets.updated_at
FROM assets
INNER JOIN laboratories USING (laboratory_id)
INNER JOIN units ON units.unit_id = assets.default_unit_id
LEFT JOIN units AS minimum_stock_units ON minimum_stock_units.unit_id = assets.minimum_stock_unit_id
LEFT JOIN asset_categories ON asset_categories.category_id = assets.category_id
WHERE assets.asset_id = $1
"#;

pub(super) fn asset_list_select() -> &'static str {
    r#"
    SELECT
        assets.asset_id,
        assets.laboratory_id,
        laboratories.name AS laboratory_name,
        assets.category_id,
        asset_categories.name AS category_name,
        assets.asset_kind,
        assets.tracking_mode,
        assets.name,
        assets.model,
        assets.manufacturer,
        assets.default_unit_id,
        units.code AS default_unit_code,
        assets.minimum_stock_quantity,
        assets.minimum_stock_unit_id,
        minimum_stock_units.code AS minimum_stock_unit_code,
        assets.public_notes,
        assets.internal_notes,
        assets.is_archived,
        assets.created_at,
        assets.updated_at
    FROM assets
    INNER JOIN laboratories USING (laboratory_id)
    INNER JOIN units ON units.unit_id = assets.default_unit_id
    LEFT JOIN units AS minimum_stock_units ON minimum_stock_units.unit_id = assets.minimum_stock_unit_id
    LEFT JOIN asset_categories ON asset_categories.category_id = assets.category_id
    "#
}

pub(super) fn resolve_target_laboratory(
    actor: &Actor,
    laboratory_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    if actor.is_system_admin() {
        return laboratory_id
            .ok_or_else(|| ApiError::BadRequest("laboratory_id is required".into()));
    }
    let actor_laboratory_id = actor.laboratory_id.ok_or(ApiError::Forbidden)?;
    if laboratory_id.is_some() && laboratory_id != Some(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    if !actor.can_write_laboratory_resource(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    Ok(actor_laboratory_id)
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_category(
    pool: &PgPool,
    laboratory_id: Uuid,
    category_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if let Some(category_id) = category_id {
        let category_laboratory_id: Option<Uuid> =
            sqlx::query_scalar("SELECT laboratory_id FROM asset_categories WHERE category_id = $1")
                .bind(category_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::UnexpectedError(e.into()))?;
        match category_laboratory_id {
            Some(category_laboratory_id) if category_laboratory_id == laboratory_id => Ok(()),
            Some(_) => Err(ApiError::BadRequest(
                "category_id belongs to another laboratory".into(),
            )),
            None => Err(ApiError::BadRequest("Unknown asset category".into())),
        }
    } else {
        Ok(())
    }
}

pub(super) async fn unit_code(pool: &PgPool, unit_id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT code FROM units WHERE unit_id = $1")
        .bind(unit_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or_else(|| ApiError::BadRequest("Unknown unit".into()))
}

pub(super) async fn default_pcs_unit_id(pool: &PgPool) -> Result<Uuid, ApiError> {
    sqlx::query_scalar("SELECT unit_id FROM units WHERE code = 'pcs'")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(value)
}

pub(super) fn normalize_asset_kind(asset_kind: &str) -> Result<&'static str, ApiError> {
    match asset_kind.trim() {
        "equipment" => Ok("equipment"),
        "material" => Ok("material"),
        "other" => Ok("other"),
        _ => Err(ApiError::BadRequest("Unknown asset_kind".into())),
    }
}

pub(super) fn normalize_tracking_mode(tracking_mode: &str) -> Result<&'static str, ApiError> {
    match tracking_mode.trim() {
        "serialized" => Ok("serialized"),
        "quantity" => Ok("quantity"),
        _ => Err(ApiError::BadRequest("Unknown tracking_mode".into())),
    }
}

pub(super) async fn validate_default_unit(
    pool: &PgPool,
    tracking_mode: &str,
    default_unit_id: Uuid,
) -> Result<(), ApiError> {
    let code = unit_code(pool, default_unit_id).await?;
    if tracking_mode == "serialized" && code != "pcs" {
        return Err(ApiError::BadRequest(
            "serialized assets must use pcs as the default unit".into(),
        ));
    }
    Ok(())
}

pub(super) fn ensure_can_manage_threshold(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<(), ApiError> {
    if actor.is_system_admin()
        || (actor.is_lab_admin() && actor.laboratory_id == Some(laboratory_id))
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_minimum_stock_threshold(
    pool: &PgPool,
    tracking_mode: &str,
    default_unit_id: Uuid,
    quantity: Option<f64>,
    unit_id: Option<Uuid>,
) -> Result<(), ApiError> {
    match (quantity, unit_id) {
        (None, None) => Ok(()),
        (Some(quantity), Some(unit_id)) => {
            if tracking_mode != "quantity" {
                return Err(ApiError::BadRequest(
                    "minimum stock thresholds only apply to quantity assets".into(),
                ));
            }
            if !quantity.is_finite() || quantity < 0.0 {
                return Err(ApiError::BadRequest(
                    "minimum_stock_quantity must be non-negative".into(),
                ));
            }
            let unit: Option<(bool, String, String)> = sqlx::query_as(
                r#"
                SELECT
                    threshold_unit.allow_decimal,
                    threshold_unit.dimension,
                    default_unit.dimension
                FROM units AS threshold_unit
                CROSS JOIN units AS default_unit
                WHERE threshold_unit.unit_id = $1
                  AND default_unit.unit_id = $2
                "#,
            )
            .bind(unit_id)
            .bind(default_unit_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
            let (allow_decimal, threshold_dimension, default_dimension) =
                unit.ok_or_else(|| ApiError::BadRequest("Unknown minimum_stock_unit_id".into()))?;
            if threshold_dimension != default_dimension {
                return Err(ApiError::BadRequest(
                    "minimum stock unit dimension does not match the asset default unit".into(),
                ));
            }
            if !allow_decimal && quantity.fract().abs() > f64::EPSILON {
                return Err(ApiError::BadRequest(
                    "minimum_stock_quantity must be an integer for this unit".into(),
                ));
            }
            Ok(())
        }
        _ => Err(ApiError::BadRequest(
            "minimum_stock_quantity and minimum_stock_unit_id must be provided together".into(),
        )),
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return ApiError::Conflict("Asset already exists".into()),
            Some("23503") => return ApiError::Conflict("Asset is still referenced".into()),
            Some("23514") => return ApiError::BadRequest("Invalid asset data".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
