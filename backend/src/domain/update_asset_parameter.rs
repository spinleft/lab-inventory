use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterName, AssetParameterOptionLabel,
    NullableUpdate, UnitDimension,
};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug)]
pub struct UpdateAssetParameterOption {
    pub option_id: Option<Uuid>,
    pub code: AssetParameterCode,
    pub label: AssetParameterOptionLabel,
    pub sort_order: i32,
}

/// Where one already stored value ends up when a parameter's data type changes.
///
/// `None` is a value the caller wants removed rather than converted. A data
/// type change has to account for every value the parameter stores, so the
/// update carries one entry per stored value or none at all.
#[derive(Debug)]
pub struct UpdateAssetParameterValueMigration {
    pub value_id: Uuid,
    pub value: Option<Value>,
}

#[derive(Debug)]
pub struct UpdateAssetParameter {
    pub code: Option<AssetParameterCode>,
    pub name: Option<AssetParameterName>,
    pub data_type: Option<AssetParameterDataType>,
    pub unit_dimension: NullableUpdate<UnitDimension>,
    pub default_unit_id: NullableUpdate<Uuid>,
    pub description: NullableUpdate<String>,
    pub options: Option<Vec<UpdateAssetParameterOption>>,
    pub value_migrations: Option<Vec<UpdateAssetParameterValueMigration>>,
}
