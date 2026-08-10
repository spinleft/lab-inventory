use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterName, AssetParameterOptionLabel,
    UnitDimension,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct NewAssetParameterOption {
    pub code: AssetParameterCode,
    pub label: AssetParameterOptionLabel,
    pub sort_order: i32,
}

#[derive(Debug)]
pub struct NewAssetParameter {
    pub code: AssetParameterCode,
    pub name: AssetParameterName,
    pub data_type: AssetParameterDataType,
    pub unit_dimension: Option<UnitDimension>,
    pub default_unit_id: Option<Uuid>,
    pub description: Option<String>,
    pub options: Vec<NewAssetParameterOption>,
}
