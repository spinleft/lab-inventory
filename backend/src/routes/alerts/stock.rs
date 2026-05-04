use super::model::{StockAlertResponse, StockAlertRow};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    laboratory_id: Option<Uuid>,
    asset_id: Option<Uuid>,
    category_id: Option<Uuid>,
}

#[tracing::instrument(name = "List stock alerts", skip(pool), fields(user_id=%user_id))]
pub async fn list_stock_alerts(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<QueryParams>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let alerts = sqlx::query_as::<_, StockAlertRow>(
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
            assets.default_unit_id,
            default_units.code AS default_unit_code,
            assets.minimum_stock_quantity AS minimum_stock_quantity,
            assets.minimum_stock_unit_id AS minimum_stock_unit_id,
            minimum_stock_units.code AS minimum_stock_unit_code,
            COALESCE(
                SUM(
                    (asset_inventory_items.quantity_on_hand - asset_inventory_items.quantity_allocated)
                    * inventory_units.scale_to_base
                    / minimum_stock_units.scale_to_base
                ),
                0
            ) AS quantity_available,
            assets.public_notes,
            assets.internal_notes
        FROM assets
        INNER JOIN laboratories USING (laboratory_id)
        INNER JOIN units AS default_units ON default_units.unit_id = assets.default_unit_id
        INNER JOIN units AS minimum_stock_units ON minimum_stock_units.unit_id = assets.minimum_stock_unit_id
        LEFT JOIN asset_categories ON asset_categories.category_id = assets.category_id
        LEFT JOIN asset_inventory_items ON asset_inventory_items.asset_id = assets.asset_id
        LEFT JOIN units AS inventory_units ON inventory_units.unit_id = asset_inventory_items.unit_id
        WHERE assets.tracking_mode = 'quantity'
          AND assets.minimum_stock_quantity IS NOT NULL
          AND ($1::uuid IS NULL OR assets.laboratory_id = $1)
          AND ($2::uuid IS NULL OR assets.asset_id = $2)
          AND ($3::uuid IS NULL OR assets.category_id = $3)
        GROUP BY
            assets.asset_id,
            laboratories.name,
            asset_categories.name,
            default_units.code,
            minimum_stock_units.code,
            minimum_stock_units.scale_to_base
        HAVING COALESCE(
            SUM(
                (asset_inventory_items.quantity_on_hand - asset_inventory_items.quantity_allocated)
                * inventory_units.scale_to_base
                / minimum_stock_units.scale_to_base
            ),
            0
        ) < assets.minimum_stock_quantity
        ORDER BY laboratories.name, assets.name, assets.model
        "#,
    )
    .bind(query.laboratory_id)
    .bind(query.asset_id)
    .bind(query.category_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .into_iter()
    .map(|alert| StockAlertResponse::from_row(alert, &actor))
    .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(alerts))
}
