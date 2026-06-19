use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetCategoryId(Uuid);

impl AssetCategoryId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for AssetCategoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for AssetCategoryId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for AssetCategoryId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetCategoryId> for Uuid {
    fn from(category_id: AssetCategoryId) -> Self {
        category_id.0
    }
}
