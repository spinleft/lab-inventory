use crate::domain::{AssetCategoryId, AssetName, AssetTrackingMode, NullableUpdate};
use uuid::Uuid;

#[derive(Debug)]
pub struct UpdateAsset {
    pub category_id: NullableUpdate<AssetCategoryId>,
    pub tracking_mode: Option<AssetTrackingMode>,
    pub name: Option<AssetName>,
    pub model: NullableUpdate<String>,
    pub manufacturer: NullableUpdate<String>,
    pub default_unit_id: Option<Uuid>,
    pub public_notes: NullableUpdate<String>,
    pub internal_notes: NullableUpdate<String>,
    pub is_archived: Option<bool>,
}

impl UpdateAsset {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        category_id: NullableUpdate<AssetCategoryId>,
        tracking_mode: Option<AssetTrackingMode>,
        name: Option<AssetName>,
        model: NullableUpdate<String>,
        manufacturer: NullableUpdate<String>,
        default_unit_id: Option<Uuid>,
        public_notes: NullableUpdate<String>,
        internal_notes: NullableUpdate<String>,
        is_archived: Option<bool>,
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
            is_archived,
        }
    }
}
