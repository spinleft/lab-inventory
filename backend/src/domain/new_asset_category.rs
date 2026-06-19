use crate::domain::{AssetCategoryCode, AssetCategoryId, AssetCategoryName};

#[derive(Debug)]
pub struct NewAssetCategory {
    pub parent_category_id: Option<AssetCategoryId>,
    pub name: AssetCategoryName,
    pub code: AssetCategoryCode,
    pub description: Option<String>,
}

impl NewAssetCategory {
    pub fn new(
        parent_category_id: Option<AssetCategoryId>,
        name: AssetCategoryName,
        code: AssetCategoryCode,
        description: Option<String>,
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
    use super::NewAssetCategory;
    use crate::domain::{AssetCategoryCode, AssetCategoryId, AssetCategoryName};
    use uuid::Uuid;

    #[test]
    fn new_asset_category_keeps_parent_name_code_and_description() {
        let parent_category_id = AssetCategoryId::parse(Uuid::new_v4()).unwrap();
        let name = AssetCategoryName::parse("Microscopes".into()).unwrap();
        let code = AssetCategoryCode::parse("microscopes".into()).unwrap();

        let category = NewAssetCategory::new(
            Some(parent_category_id),
            name,
            code,
            Some("Optical devices".into()),
        );

        assert_eq!(category.parent_category_id, Some(parent_category_id));
        assert_eq!(category.name.as_ref(), "Microscopes");
        assert_eq!(category.code.as_ref(), "microscopes");
        assert_eq!(category.description.as_deref(), Some("Optical devices"));
    }
}
