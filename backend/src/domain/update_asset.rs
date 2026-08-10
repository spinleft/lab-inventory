use crate::domain::{AssetCategoryId, AssetName, AssetTrackingMode, NullableUpdate, UnitId};

#[derive(Debug)]
pub struct UpdateAsset {
    pub category_id: NullableUpdate<AssetCategoryId>,
    pub tracking_mode: Option<AssetTrackingMode>,
    pub name: Option<AssetName>,
    pub model: NullableUpdate<String>,
    pub manufacturer: NullableUpdate<String>,
    pub inventory_unit_id: Option<UnitId>,
    pub public_notes: NullableUpdate<String>,
    pub internal_notes: NullableUpdate<String>,
}
