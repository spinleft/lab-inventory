//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
use super::model::{
    AssetInventoryItemRow, AssetParameterDefinitionRow, AssetParameterValueInput,
    ResolvedAssetParameterValue,
};
use super::queries::{
    self, AssetDatabaseError, delete_asset_parameter_value,
    fetch_inventory_items_for_asset_for_update, fetch_parameter_definitions,
    fetch_required_parameters, fetch_unit, insert_inventory_item, update_inventory_item_quantities,
    upsert_asset_parameter_value, validate_location, validate_option,
};
use crate::domain::{
    AssetParameterDataType, AssetParameterValue, AssetTrackingMode, LaboratoryId, NewInventoryItem,
};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

/// Inserts every inventory item of a new asset, checking that each referenced
/// location belongs to the same laboratory.
pub(super) async fn insert_inventory_items(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    tracking_mode: AssetTrackingMode,
    items: &[NewInventoryItem],
) -> Result<Vec<AssetInventoryItemRow>, AssetDatabaseError> {
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        if let Some(location_id) = item.location_id {
            validate_location(transaction, laboratory_id, location_id.into()).await?;
        }
        rows.push(
            insert_inventory_item(transaction, laboratory_id, asset_id, tracking_mode, item)
                .await?,
        );
    }

    Ok(rows)
}

/// Rewrites every inventory item of an asset into `target_unit_id` after the
/// asset's default unit changed.
pub(super) async fn convert_inventory_items_to_unit(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    target_unit_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    let items = fetch_inventory_items_for_asset_for_update(transaction, asset_id).await?;

    for item in items {
        if item.quantity_unit_id == target_unit_id {
            continue;
        }

        let quantity_on_hand = convert_quantity_between_units(
            transaction,
            item.quantity_unit_id,
            target_unit_id,
            item.quantity_on_hand,
        )
        .await?;
        let quantity_allocated = convert_quantity_between_units(
            transaction,
            item.quantity_unit_id,
            target_unit_id,
            item.quantity_allocated,
        )
        .await?;

        update_inventory_item_quantities(
            transaction,
            item.inventory_item_id,
            quantity_on_hand,
            quantity_allocated,
            target_unit_id,
        )
        .await?;
    }

    Ok(())
}

async fn convert_quantity_between_units(
    transaction: &mut Transaction<'_, Postgres>,
    source_unit_id: Uuid,
    target_unit_id: Uuid,
    source_quantity: f64,
) -> Result<f64, AssetDatabaseError> {
    if source_unit_id == target_unit_id {
        return Ok(source_quantity);
    }
    let source_unit = fetch_unit(transaction, source_unit_id).await?;
    let target_unit = fetch_unit(transaction, target_unit_id).await?;
    if source_unit.dimension != target_unit.dimension {
        return Err(AssetDatabaseError::Validation(
            "Asset default unit dimension does not match inventory item unit dimension".into(),
        ));
    }

    Ok(source_quantity * source_unit.scale_to_base / target_unit.scale_to_base)
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

async fn resolve_parameter_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &AssetParameterDefinitionRow,
    value: &Value,
) -> Result<ResolvedAssetParameterValue, AssetDatabaseError> {
    let data_type = AssetParameterDataType::parse(&definition.data_type)
        .map_err(|_| AssetDatabaseError::Validation("Invalid asset parameter data type".into()))?;
    let value =
        AssetParameterValue::parse(data_type, value).map_err(AssetDatabaseError::Validation)?;
    let mut resolved = ResolvedAssetParameterValue {
        parameter_type_id: definition.parameter_type_id,
        data_type: definition.data_type.clone(),
        value_text: None,
        value_number: None,
        value_number_base: None,
        value_range_start: None,
        value_range_end: None,
        value_range_start_base: None,
        value_range_end_base: None,
        unit_id: None,
        value_boolean: None,
        value_date: None,
        value_option_id: None,
    };

    match value {
        AssetParameterValue::Text(text) => resolved.value_text = Some(text),
        AssetParameterValue::Number { number, unit_id } => {
            let (unit_id, number_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), number)
                    .await?;
            resolved.value_number = Some(number);
            resolved.value_number_base = number_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Range {
            start,
            end,
            unit_id,
        } => {
            let (unit_id, start_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), start)
                    .await?;
            let (_, end_base) = normalize_unit_value(transaction, definition, unit_id, end).await?;
            resolved.value_range_start = Some(start);
            resolved.value_range_end = Some(end);
            resolved.value_range_start_base = start_base;
            resolved.value_range_end_base = end_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Boolean(boolean) => resolved.value_boolean = Some(boolean),
        AssetParameterValue::Date(date) => resolved.value_date = Some(date),
        AssetParameterValue::Enum(option_id) => {
            validate_option(transaction, definition.parameter_type_id, option_id).await?;
            resolved.value_option_id = Some(option_id);
        }
    }

    Ok(resolved)
}

async fn normalize_unit_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &AssetParameterDefinitionRow,
    unit_id: Option<Uuid>,
    value: f64,
) -> Result<(Option<Uuid>, Option<f64>), AssetDatabaseError> {
    let unit_id = unit_id.or(definition.default_unit_id);
    let Some(unit_id) = unit_id else {
        return Ok((None, None));
    };
    let unit = fetch_unit(transaction, unit_id).await?;

    match definition.unit_dimension.as_deref() {
        Some(unit_dimension) if unit_dimension == unit.dimension => {
            Ok((Some(unit.unit_id), Some(value * unit.scale_to_base)))
        }
        Some(_) => Err(AssetDatabaseError::Validation(
            "Parameter value unit dimension does not match parameter definition".into(),
        )),
        None => Err(AssetDatabaseError::Validation(
            "Parameter value unit is not allowed for this parameter".into(),
        )),
    }
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
