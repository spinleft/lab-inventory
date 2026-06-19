use crate::domain::{AssetCategoryCode, AssetCategoryId, AssetCategoryName, NullableUpdate};

#[derive(Debug)]
pub struct UpdateAssetCategory {
    pub parent_category_id: NullableUpdate<AssetCategoryId>,
    pub name: Option<AssetCategoryName>,
    pub code: Option<AssetCategoryCode>,
    pub description: NullableUpdate<String>,
}

impl UpdateAssetCategory {
    pub fn new(
        parent_category_id: NullableUpdate<AssetCategoryId>,
        name: Option<AssetCategoryName>,
        code: Option<AssetCategoryCode>,
        description: NullableUpdate<String>,
    ) -> Self {
        Self {
            parent_category_id,
            name,
            code,
            description,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateAssetCategory;
    use crate::domain::{AssetCategoryCode, AssetCategoryId, AssetCategoryName, NullableUpdate};
    use uuid::Uuid;

    #[test]
    fn update_asset_category_captures_partial_updates() {
        let parent_category_id = AssetCategoryId::parse(Uuid::new_v4()).unwrap();
        let name = AssetCategoryName::parse("Microscopes".into()).unwrap();
        let code = AssetCategoryCode::parse("microscopes".into()).unwrap();

        let update = UpdateAssetCategory::new(
            NullableUpdate::Set(parent_category_id),
            Some(name),
            Some(code),
            NullableUpdate::Clear,
        );

        assert!(matches!(
            update.parent_category_id,
            NullableUpdate::Set(value) if value == parent_category_id
        ));
        assert_eq!(
            update.name.as_ref().map(|name| name.as_ref()),
            Some("Microscopes")
        );
        assert_eq!(
            update.code.as_ref().map(|code| code.as_ref()),
            Some("microscopes")
        );
        assert!(matches!(update.description, NullableUpdate::Clear));
    }
}
