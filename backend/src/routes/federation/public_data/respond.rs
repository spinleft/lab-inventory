//! Turns a federation read path into an HTTP response.
//!
//! The inbound endpoint takes an arbitrary tail path rather than a set of typed
//! routes, so the routing that Actix would normally do happens here:
//! [`parse_read_target`] resolves the tail, and [`respond_public_data`] hands
//! the target to `service.rs` and serializes what comes back.
use super::model::FederationReadTarget;
use super::service;
use crate::domain::FileStorageKey;
use crate::file_storage::FileStorage;
use crate::routes::federation::model::FederationError;
use actix_web::HttpResponse;
use actix_web::http::header;
use sqlx::PgPool;
use uuid::Uuid;

pub(crate) fn parse_read_target(tail: &str) -> Result<FederationReadTarget, FederationError> {
    let mut parts = tail
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty());
    let first = parts.next();
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return Err(FederationError::NotFound(
            "Federation route not found".into(),
        ));
    }
    match (first, second, third) {
        (None, None, None) => Ok(FederationReadTarget::Laboratory),
        (Some("assets"), None, None) => Ok(FederationReadTarget::Assets),
        (Some("assets"), Some(asset_id), None) => {
            Ok(FederationReadTarget::Asset(parse_uuid(asset_id)?))
        }
        (Some("assets"), Some(asset_id), Some("attachments")) => Ok(
            FederationReadTarget::AssetAttachments(parse_uuid(asset_id)?),
        ),
        (Some("inventory-items"), None, None) => Ok(FederationReadTarget::InventoryItems),
        (Some("inventory-items"), Some(item_id), None) => {
            Ok(FederationReadTarget::InventoryItem(parse_uuid(item_id)?))
        }
        (Some("inventory-items"), Some(item_id), Some("attachments")) => Ok(
            FederationReadTarget::InventoryItemAttachments(parse_uuid(item_id)?),
        ),
        (Some("asset-categories"), None, None) => Ok(FederationReadTarget::AssetCategories),
        (Some("asset-categories"), Some(category_id), None) => Ok(
            FederationReadTarget::AssetCategory(parse_uuid(category_id)?),
        ),
        (Some("asset-parameters"), None, None) => Ok(FederationReadTarget::AssetParameters),
        (Some("asset-parameters"), Some(parameter_id), None) => Ok(
            FederationReadTarget::AssetParameter(parse_uuid(parameter_id)?),
        ),
        (Some("locations"), None, None) => Ok(FederationReadTarget::Locations),
        (Some("locations"), Some(location_id), None) => {
            Ok(FederationReadTarget::Location(parse_uuid(location_id)?))
        }
        (Some("units"), None, None) => Ok(FederationReadTarget::Units),
        (Some("units"), Some(unit_id), None) => {
            Ok(FederationReadTarget::Unit(parse_uuid(unit_id)?))
        }
        (Some("attachments"), None, None) => Ok(FederationReadTarget::Attachments),
        (Some("attachments"), Some(attachment_id), None) => {
            Ok(FederationReadTarget::Attachment(parse_uuid(attachment_id)?))
        }
        (Some("attachments"), Some(attachment_id), Some("download")) => Ok(
            FederationReadTarget::AttachmentDownload(parse_uuid(attachment_id)?),
        ),
        _ => Err(FederationError::NotFound(
            "Federation route not found".into(),
        )),
    }
}

pub(crate) async fn respond_public_data(
    pool: &PgPool,
    storage: &FileStorage,
    laboratory_id: Uuid,
    target: FederationReadTarget,
    query_string: &str,
) -> Result<HttpResponse, FederationError> {
    match target {
        FederationReadTarget::Laboratory => {
            Ok(HttpResponse::Ok().json(service::fetch_laboratory(pool, laboratory_id).await?))
        }
        FederationReadTarget::Assets => {
            Ok(HttpResponse::Ok()
                .json(service::list_assets(pool, laboratory_id, query_string).await?))
        }
        FederationReadTarget::Asset(asset_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_asset(pool, laboratory_id, asset_id, query_string).await?)),
        FederationReadTarget::InventoryItems => Ok(HttpResponse::Ok()
            .json(service::list_inventory_items(pool, laboratory_id, query_string).await?)),
        FederationReadTarget::InventoryItem(item_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_inventory_item(pool, laboratory_id, item_id).await?)),
        FederationReadTarget::AssetCategories => Ok(HttpResponse::Ok()
            .json(service::list_categories(pool, laboratory_id, query_string).await?)),
        FederationReadTarget::AssetCategory(category_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_category(pool, laboratory_id, category_id).await?)),
        FederationReadTarget::Locations => Ok(HttpResponse::Ok()
            .json(service::list_locations(pool, laboratory_id, query_string).await?)),
        FederationReadTarget::Location(location_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_location(pool, laboratory_id, location_id).await?)),
        FederationReadTarget::Units => {
            Ok(HttpResponse::Ok().json(service::list_units(pool, laboratory_id).await?))
        }
        FederationReadTarget::Unit(unit_id) => {
            Ok(HttpResponse::Ok().json(service::fetch_unit(pool, laboratory_id, unit_id).await?))
        }
        FederationReadTarget::AssetParameters => {
            Ok(HttpResponse::Ok().json(service::list_parameters(pool, laboratory_id).await?))
        }
        FederationReadTarget::AssetParameter(parameter_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_parameter(pool, laboratory_id, parameter_id).await?)),
        FederationReadTarget::Attachments => Ok(HttpResponse::Ok()
            .json(service::list_laboratory_attachments(pool, laboratory_id, query_string).await?)),
        FederationReadTarget::Attachment(attachment_id) => Ok(HttpResponse::Ok()
            .json(service::fetch_attachment(pool, laboratory_id, attachment_id).await?)),
        FederationReadTarget::AssetAttachments(asset_id) => Ok(HttpResponse::Ok()
            .json(service::list_asset_attachments(pool, laboratory_id, asset_id).await?)),
        FederationReadTarget::InventoryItemAttachments(item_id) => Ok(HttpResponse::Ok()
            .json(service::list_inventory_item_attachments(pool, laboratory_id, item_id).await?)),
        FederationReadTarget::AttachmentDownload(attachment_id) => {
            download_attachment(pool, storage, laboratory_id, attachment_id).await
        }
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, FederationError> {
    value
        .parse()
        .map_err(|_| FederationError::NotFound("Federation route not found".into()))
}

async fn download_attachment(
    pool: &PgPool,
    storage: &FileStorage,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<HttpResponse, FederationError> {
    let row = service::fetch_attachment_download(pool, laboratory_id, attachment_id).await?;
    let storage_key = FileStorageKey::parse(row.storage_key)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!("{e}")))?;
    let bytes = storage
        .read(&storage_key)
        .await
        .map_err(FederationError::UnexpectedError)?;

    Ok(HttpResponse::Ok()
        .insert_header((
            header::CONTENT_TYPE,
            row.mime_type
                .unwrap_or_else(|| "application/octet-stream".to_string()),
        ))
        .insert_header((header::CONTENT_LENGTH, bytes.len().to_string()))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                content_disposition_filename(&row.original_file_name)
            ),
        ))
        .body(bytes))
}

fn content_disposition_filename(file_name: &str) -> String {
    file_name
        .chars()
        .map(|ch| match ch {
            '"' | '\\' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
}
