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
    pub is_archived: bool,
}

impl UpdateAssetParameterOption {
    pub fn new(
        option_id: Option<Uuid>,
        code: AssetParameterCode,
        label: AssetParameterOptionLabel,
        sort_order: i32,
        is_archived: bool,
    ) -> Self {
        Self {
            option_id,
            code,
            label,
            sort_order,
            is_archived,
        }
    }
}

#[derive(Debug)]
pub struct UpdateAssetParameter {
    pub code: Option<AssetParameterCode>,
    pub name: Option<AssetParameterName>,
    pub data_type: Option<AssetParameterDataType>,
    pub unit_dimension: NullableUpdate<UnitDimension>,
    pub default_unit_id: NullableUpdate<Uuid>,
    pub description: NullableUpdate<String>,
    pub is_archived: Option<bool>,
    pub options: Option<Vec<UpdateAssetParameterOption>>,
}

impl UpdateAssetParameter {
    pub fn new(
        code: Option<AssetParameterCode>,
        name: Option<AssetParameterName>,
        data_type: Option<AssetParameterDataType>,
        unit_dimension: NullableUpdate<UnitDimension>,
        default_unit_id: NullableUpdate<Uuid>,
        description: NullableUpdate<String>,
        is_archived: Option<bool>,
        options: Option<Vec<UpdateAssetParameterOption>>,
    ) -> Self {
        Self {
            code,
            name,
            data_type,
            unit_dimension,
            default_unit_id,
            description,
            is_archived,
            options,
        }
    }
}
