//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and assembles the shapes the public
//! API answers with. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in `respond.rs`.
use super::model::{
    AssetPublicResponse, AttachmentDownloadRow, AttachmentPublicRow, CategoryRow,
    InventoryItemPublicResponse, LaboratoryPublicRow, LocationRow, PaginatedJson,
    ParameterResponse, ParameterRow, ParameterValueResponse,
};
use super::queries;
use crate::routes::federation::model::FederationError;
use sqlx::PgPool;
use std::collections::HashMap;
use url::form_urlencoded;
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

// ---------------------------------------------------------------------------
// query string
// ---------------------------------------------------------------------------

/// The federation API is reached through a raw tail path rather than a typed
/// handler, so its filters arrive as an unparsed query string.
pub(super) fn query_params(query_string: &str) -> HashMap<String, String> {
    form_urlencoded::parse(query_string.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

pub(super) fn limit_offset(query_string: &str) -> Result<(i64, i64), FederationError> {
    let params = query_params(query_string);
    let limit = params
        .get("limit")
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| FederationError::ValidationError("limit must be a number".into()))?
        .unwrap_or(DEFAULT_LIMIT);
    let offset = params
        .get("offset")
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| FederationError::ValidationError("offset must be a number".into()))?
        .unwrap_or(0);
    if limit <= 0 {
        return Err(FederationError::ValidationError(
            "limit must be positive".into(),
        ));
    }
    if offset < 0 {
        return Err(FederationError::ValidationError(
            "offset must be non-negative".into(),
        ));
    }

    Ok((limit.min(MAX_LIMIT), offset))
}

// ---------------------------------------------------------------------------
// reads
// ---------------------------------------------------------------------------

pub(super) async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<LaboratoryPublicRow, FederationError> {
    queries::fetch_laboratory(pool, laboratory_id).await
}

pub(super) async fn list_assets(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<AssetPublicResponse>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let params = query_params(query_string);
    let total = queries::count_assets(pool, laboratory_id, &params).await?;
    let rows = queries::fetch_assets(pool, laboratory_id, &params, limit, offset).await?;

    Ok(PaginatedJson {
        // A listing carries neither the items nor the parameters of an asset;
        // those are only worth the extra queries on a single asset.
        items: rows
            .into_iter()
            .map(|row| AssetPublicResponse::from_row(row, None, None))
            .collect(),
        limit,
        offset,
        total,
    })
}

/// One asset with everything a remote reader is allowed to see of it: always its
/// inventory items, and its parameter values when asked for with
/// `include=parameters`.
pub(super) async fn fetch_asset(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
    query_string: &str,
) -> Result<AssetPublicResponse, FederationError> {
    let row = queries::fetch_asset(pool, laboratory_id, asset_id).await?;
    let inventory_items = list_inventory_items_for_asset(pool, laboratory_id, asset_id).await?;
    let include_parameters = query_params(query_string)
        .get("include")
        .is_some_and(|value| value.split(',').any(|part| part.trim() == "parameters"));
    let parameters = if include_parameters {
        Some(
            fetch_parameter_values(pool, &[asset_id])
                .await?
                .remove(&asset_id)
                .unwrap_or_default(),
        )
    } else {
        None
    };

    Ok(AssetPublicResponse::from_row(
        row,
        Some(inventory_items),
        parameters,
    ))
}

pub(super) async fn list_inventory_items(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<InventoryItemPublicResponse>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let params = query_params(query_string);
    let total = queries::count_inventory_items(pool, laboratory_id, &params).await?;
    let rows = queries::fetch_inventory_items(pool, laboratory_id, &params, limit, offset).await?;

    Ok(PaginatedJson {
        items: rows
            .into_iter()
            .map(InventoryItemPublicResponse::from)
            .collect(),
        limit,
        offset,
        total,
    })
}

pub(super) async fn fetch_inventory_item(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<InventoryItemPublicResponse, FederationError> {
    let row = queries::fetch_inventory_item(pool, laboratory_id, inventory_item_id).await?;

    Ok(InventoryItemPublicResponse::from(row))
}

async fn list_inventory_items_for_asset(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<InventoryItemPublicResponse>, FederationError> {
    let rows = queries::fetch_inventory_items_for_asset(pool, laboratory_id, asset_id).await?;

    Ok(rows
        .into_iter()
        .map(InventoryItemPublicResponse::from)
        .collect())
}

/// `root_category_id` narrows the tree to one subtree, which is expressed as a
/// path prefix — so the root has to be looked up before the tree can be read.
pub(super) async fn list_categories(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<Vec<CategoryRow>, FederationError> {
    let params = query_params(query_string);
    let root_path = match params
        .get("root_category_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        Some(root_id) => Some(queries::fetch_category(pool, laboratory_id, root_id).await?.path),
        None => None,
    };

    queries::fetch_categories(pool, laboratory_id, root_path).await
}

pub(super) async fn fetch_category(
    pool: &PgPool,
    laboratory_id: Uuid,
    category_id: Uuid,
) -> Result<CategoryRow, FederationError> {
    queries::fetch_category(pool, laboratory_id, category_id).await
}

pub(super) async fn list_locations(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<Vec<LocationRow>, FederationError> {
    let params = query_params(query_string);
    let root_path = match params
        .get("root_location_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        Some(root_id) => Some(queries::fetch_location(pool, laboratory_id, root_id).await?.path),
        None => None,
    };

    queries::fetch_locations(pool, laboratory_id, root_path).await
}

pub(super) async fn fetch_location(
    pool: &PgPool,
    laboratory_id: Uuid,
    location_id: Uuid,
) -> Result<LocationRow, FederationError> {
    queries::fetch_location(pool, laboratory_id, location_id).await
}

pub(super) async fn list_parameters(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<ParameterResponse>, FederationError> {
    let rows = queries::fetch_parameters(pool, laboratory_id).await?;
    let mut response = Vec::with_capacity(rows.len());
    for row in rows {
        response.push(parameter_response(pool, row).await?);
    }

    Ok(response)
}

pub(super) async fn fetch_parameter(
    pool: &PgPool,
    laboratory_id: Uuid,
    parameter_id: Uuid,
) -> Result<ParameterResponse, FederationError> {
    let row = queries::fetch_parameter(pool, laboratory_id, parameter_id).await?;

    parameter_response(pool, row).await
}

/// A parameter is only fully described together with the options it allows, so
/// they are read alongside it.
async fn parameter_response(
    pool: &PgPool,
    row: ParameterRow,
) -> Result<ParameterResponse, FederationError> {
    let options = queries::fetch_parameter_options(pool, row.parameter_type_id).await?;

    Ok(ParameterResponse {
        parameter_type_id: row.parameter_type_id,
        laboratory_id: row.laboratory_id,
        code: row.code,
        name: row.name,
        data_type: row.data_type,
        unit_dimension: row.unit_dimension,
        default_unit_id: row.default_unit_id,
        description: row.description,
        options,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn fetch_parameter_values(
    pool: &PgPool,
    asset_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<ParameterValueResponse>>, FederationError> {
    let rows = queries::fetch_parameter_values(pool, asset_ids).await?;
    let mut values: HashMap<Uuid, Vec<_>> = HashMap::new();
    for row in rows {
        values
            .entry(row.asset_id)
            .or_default()
            .push(ParameterValueResponse::from(row));
    }

    Ok(values)
}

pub(super) async fn list_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<AttachmentPublicRow>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let total = queries::count_laboratory_attachments(pool, laboratory_id).await?;
    let items = queries::fetch_laboratory_attachments(pool, laboratory_id, limit, offset).await?;

    Ok(PaginatedJson {
        items,
        limit,
        offset,
        total,
    })
}

pub(super) async fn list_asset_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, FederationError> {
    queries::fetch_asset_attachments(pool, laboratory_id, asset_id).await
}

pub(super) async fn list_inventory_item_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, FederationError> {
    queries::fetch_inventory_item_attachments(pool, laboratory_id, inventory_item_id).await
}

pub(super) async fn fetch_attachment(
    pool: &PgPool,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<AttachmentPublicRow, FederationError> {
    queries::fetch_attachment(pool, laboratory_id, attachment_id).await
}

pub(super) async fn fetch_attachment_download(
    pool: &PgPool,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<AttachmentDownloadRow, FederationError> {
    queries::fetch_attachment_download(pool, laboratory_id, attachment_id).await
}
