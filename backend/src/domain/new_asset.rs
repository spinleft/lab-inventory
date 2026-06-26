use crate::domain::{AssetCategoryId, AssetName, AssetTrackingMode};
use uuid::Uuid;

#[derive(Debug)]
pub struct NewAsset {
    pub category_id: Option<AssetCategoryId>,
    pub tracking_mode: AssetTrackingMode,
    pub name: AssetName,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub default_unit_id: Uuid,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

impl NewAsset {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        category_id: Option<AssetCategoryId>,
        tracking_mode: AssetTrackingMode,
        name: AssetName,
        model: Option<String>,
        manufacturer: Option<String>,
        default_unit_id: Uuid,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Self {
        Self {
            category_id,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            public_notes,
            internal_notes,
        }
    }
}
