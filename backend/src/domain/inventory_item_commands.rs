use crate::domain::{
    InventoryItemId, InventoryStatus, LocationId, NullableUpdate, normalize_optional_text,
    nullable_text,
};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct InventoryItemIds(Vec<InventoryItemId>);

impl InventoryItemIds {
    pub fn parse(values: Vec<Uuid>) -> Result<Self, String> {
        if values.is_empty() {
            return Err("inventory_item_ids cannot be empty".into());
        }
        let unique: HashSet<_> = values.iter().copied().collect();
        if unique.len() != values.len() {
            return Err("inventory_item_ids cannot contain duplicates".into());
        }
        Ok(Self(values.into_iter().map(Into::into).collect()))
    }

    pub fn as_slice(&self) -> &[InventoryItemId] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<InventoryItemId> {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct SplitInventoryItem {
    pub quantity: f64,
    pub batch_number: NullableUpdate<String>,
    pub location_id: NullableUpdate<LocationId>,
    pub status: Option<InventoryStatus>,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

impl SplitInventoryItem {
    pub fn parse(
        quantity: f64,
        batch_number: Option<Option<String>>,
        location_id: Option<Option<LocationId>>,
        status: Option<String>,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Result<Self, String> {
        if !quantity.is_finite() || quantity <= 0.0 {
            return Err("quantity must be positive and finite".into());
        }
        Ok(Self {
            quantity,
            batch_number: nullable_text(batch_number),
            location_id: location_id.into(),
            status: status.as_deref().map(InventoryStatus::parse).transpose()?,
            public_notes: normalize_optional_text(public_notes),
            internal_notes: normalize_optional_text(internal_notes),
        })
    }
}

#[derive(Clone, Debug)]
pub struct MergeInventoryItems {
    pub target_inventory_item_id: InventoryItemId,
    pub source_inventory_item_ids: InventoryItemIds,
}

impl MergeInventoryItems {
    pub fn parse(
        target_inventory_item_id: Uuid,
        source_inventory_item_ids: Vec<Uuid>,
    ) -> Result<Self, String> {
        let sources = InventoryItemIds::parse(source_inventory_item_ids)?;
        if sources
            .as_slice()
            .iter()
            .any(|source| Uuid::from(*source) == target_inventory_item_id)
        {
            return Err(
                "target_inventory_item_id cannot be included in source_inventory_item_ids".into(),
            );
        }
        Ok(Self {
            target_inventory_item_id: target_inventory_item_id.into(),
            source_inventory_item_ids: sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{InventoryItemIds, MergeInventoryItems, SplitInventoryItem};
    use uuid::Uuid;

    #[test]
    fn id_sets_must_be_non_empty_and_unique() {
        assert!(InventoryItemIds::parse(Vec::new()).is_err());
        let id = Uuid::new_v4();
        assert!(InventoryItemIds::parse(vec![id, id]).is_err());
    }

    #[test]
    fn split_quantity_must_be_positive_and_finite() {
        assert!(SplitInventoryItem::parse(0.0, None, None, None, None, None).is_err());
        assert!(SplitInventoryItem::parse(f64::NAN, None, None, None, None, None).is_err());
    }

    #[test]
    fn merge_sources_cannot_include_target() {
        let target = Uuid::new_v4();
        assert!(MergeInventoryItems::parse(target, vec![target]).is_err());
    }
}
