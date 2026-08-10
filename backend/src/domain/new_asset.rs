use crate::domain::{AssetCategoryId, AssetName, AssetTrackingMode, UnitId};

#[derive(Debug)]
pub struct NewAsset {
    pub category_id: Option<AssetCategoryId>,
    pub tracking_mode: AssetTrackingMode,
    pub name: AssetName,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub inventory_unit_id: UnitId,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}
