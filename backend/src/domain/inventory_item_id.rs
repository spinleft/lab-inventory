use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InventoryItemId(Uuid);

impl InventoryItemId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for InventoryItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for InventoryItemId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for InventoryItemId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<InventoryItemId> for Uuid {
    fn from(inventory_item_id: InventoryItemId) -> Self {
        inventory_item_id.0
    }
}

impl From<Uuid> for InventoryItemId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}
