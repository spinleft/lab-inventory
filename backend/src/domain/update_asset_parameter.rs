use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterName, AssetParameterOptionLabel,
    NullableUpdate, UnitDimension,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct UpdateAssetParameterOption {
    pub option_id: Option<Uuid>,
    pub code: AssetParameterCode,
    pub label: AssetParameterOptionLabel,
    pub sort_order: i32,
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
}
