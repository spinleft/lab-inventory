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

impl NewAssetParameterOption {
    pub fn new(
        code: AssetParameterCode,
        label: AssetParameterOptionLabel,
        sort_order: i32,
    ) -> Self {
        Self {
            code,
            label,
            sort_order,
        }
    }
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

impl NewAssetParameter {
    pub fn new(
        code: AssetParameterCode,
        name: AssetParameterName,
        data_type: AssetParameterDataType,
        unit_dimension: Option<UnitDimension>,
        default_unit_id: Option<Uuid>,
        description: Option<String>,
        options: Vec<NewAssetParameterOption>,
    ) -> Self {
        Self {
            code,
            name,
            data_type,
            unit_dimension,
            default_unit_id,
            description,
            options,
        }
    }
}
