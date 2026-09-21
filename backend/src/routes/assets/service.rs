//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
use super::model::{AssetInventoryItemRow, AssetParameterValueInput};
use super::queries::{
    self, AssetDatabaseError, delete_asset_parameter_value, fetch_parameter_definitions,
    fetch_required_parameters, insert_inventory_item, rescale_inventory_quantities,
    upsert_asset_parameter_value, validate_location,
};
use crate::domain::{AssetTrackingMode, LaboratoryId, NewInventoryItem};
use crate::routes::shared::parameter_values::{
    ParameterValueError, fetch_unit, resolve_parameter_value,
};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

impl From<ParameterValueError> for AssetDatabaseError {
    fn from(error: ParameterValueError) -> Self {
        match error {
            ParameterValueError::Validation(message) => Self::Validation(message),
            ParameterValueError::Unexpected(error) => Self::Unexpected(error),
        }
    }
}

/// Inserts one inventory item of a new asset, checking that its location belongs
/// to the same laboratory.
pub(super) async fn insert_asset_inventory_item(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    tracking_mode: AssetTrackingMode,
    item: &NewInventoryItem,
) -> Result<AssetInventoryItemRow, AssetDatabaseError> {
    if let Some(location_id) = item.location_id {
        validate_location(transaction, laboratory_id, location_id.into()).await?;
    }

    insert_inventory_item(transaction, laboratory_id, asset_id, tracking_mode, item).await
}

/// Keeps the recorded quantities meaning the same physical amount after the
/// asset's inventory unit changed.
///
/// Serialized assets are left alone: one item is one physical thing, its
/// `quantity_on_hand` is pinned to 1 by a table constraint, and the unit is only
/// a label for it. The dimension check still runs so the new unit cannot measure
/// something unrelated.
pub(super) async fn convert_inventory_quantities_to_unit(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    tracking_mode: AssetTrackingMode,
    source_unit_id: Uuid,
    target_unit_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    if source_unit_id == target_unit_id {
        return Ok(());
    }
    let source_unit = fetch_unit(transaction, source_unit_id).await?;
    let target_unit = fetch_unit(transaction, target_unit_id).await?;
    if source_unit.dimension != target_unit.dimension {
        return Err(AssetDatabaseError::Validation(
            "New inventory unit must measure the same dimension as the current one".into(),
        ));
    }
    if tracking_mode == AssetTrackingMode::Serialized {
        return Ok(());
    }

    let factor = source_unit.scale_to_base / target_unit.scale_to_base;
    rescale_inventory_quantities(transaction, asset_id, factor).await
}

/// Applies a batch of parameter value writes. With `allow_delete` a `None`
/// value clears the parameter; otherwise it is rejected as missing input.
pub(super) async fn apply_asset_parameter_updates(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    inputs: &[AssetParameterValueInput],
    allow_delete: bool,
) -> Result<(), AssetDatabaseError> {
    validate_unique_parameter_inputs(inputs)?;
    let parameter_type_ids: Vec<_> = inputs.iter().map(|input| input.parameter_type_id).collect();
    let definitions =
        fetch_parameter_definitions(transaction, laboratory_id, &parameter_type_ids).await?;
    for input in inputs {
        match input.value.as_ref() {
            Some(value) => {
                let definition = definitions.get(&input.parameter_type_id).ok_or(
                    AssetDatabaseError::Validation(
                        "Asset parameter does not belong to this laboratory".into(),
                    ),
                )?;
                let resolved = resolve_parameter_value(transaction, definition, value).await?;
                upsert_asset_parameter_value(transaction, laboratory_id, asset_id, &resolved)
                    .await?;
            }
            None => {
                if !allow_delete {
                    return Err(AssetDatabaseError::Validation(
                        "Asset parameter value is required".into(),
                    ));
                }
                delete_asset_parameter_value(transaction, asset_id, input.parameter_type_id)
                    .await?;
            }
        }
    }

    Ok(())
}

fn validate_unique_parameter_inputs(
    inputs: &[AssetParameterValueInput],
) -> Result<(), AssetDatabaseError> {
    let mut seen = HashSet::new();
    for input in inputs {
        if !seen.insert(input.parameter_type_id) {
            return Err(AssetDatabaseError::Validation(
                "Asset parameter values must be unique".into(),
            ));
        }
    }

    Ok(())
}

/// Every parameter the asset's category marks as required must have a value.
pub(super) async fn validate_required_parameters(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    category_id: Option<Uuid>,
) -> Result<(), AssetDatabaseError> {
    let Some(category_id) = category_id else {
        return Ok(());
    };
    let required = fetch_required_parameters(transaction, laboratory_id, category_id).await?;
    if required.is_empty() {
        return Ok(());
    }

    let existing =
        queries::fetch_parameter_type_ids_with_values(transaction, laboratory_id, asset_id).await?;

    for parameter_type_id in required {
        if !existing.contains(&parameter_type_id) {
            return Err(AssetDatabaseError::Validation(
                "Missing required asset parameter value".into(),
            ));
        }
    }

    Ok(())
}
