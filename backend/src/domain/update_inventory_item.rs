use crate::domain::{
    InventoryItemSerialNumber, InventoryStatus, LocationId, NullableUpdate, UnitId,
    normalize_optional_text, validate_quantities,
};

#[derive(Clone, Debug)]
pub struct UpdateInventoryItem {
    pub serial_number: Option<InventoryItemSerialNumber>,
    pub batch_number: NullableUpdate<String>,
    pub quantity_on_hand: Option<f64>,
    pub quantity_allocated: Option<f64>,
    pub quantity_unit_id: Option<UnitId>,
    pub location_id: NullableUpdate<LocationId>,
    pub status: Option<InventoryStatus>,
    pub public_notes: NullableUpdate<String>,
    pub internal_notes: NullableUpdate<String>,
}

impl UpdateInventoryItem {
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        serial_number: Option<String>,
        batch_number: Option<Option<String>>,
        quantity_on_hand: Option<f64>,
        quantity_allocated: Option<f64>,
        quantity_unit_id: Option<UnitId>,
        location_id: Option<Option<LocationId>>,
        status: Option<String>,
        public_notes: Option<Option<String>>,
        internal_notes: Option<Option<String>>,
    ) -> Result<Self, String> {
        if let Some(quantity) = quantity_on_hand {
            validate_quantities(quantity, 0.0)?;
        }
        if let Some(quantity) = quantity_allocated {
            validate_quantities(quantity, quantity)?;
        }
        Ok(Self {
            serial_number: serial_number
                .map(InventoryItemSerialNumber::parse)
                .transpose()?,
            batch_number: nullable_text(batch_number),
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id,
            location_id: location_id.into(),
            status: status.as_deref().map(InventoryStatus::parse).transpose()?,
            public_notes: nullable_text(public_notes),
            internal_notes: nullable_text(internal_notes),
        })
    }

    pub fn has_batch_updates(&self) -> bool {
        !matches!(self.batch_number, NullableUpdate::Unchanged)
            || !matches!(self.location_id, NullableUpdate::Unchanged)
            || self.status.is_some()
            || !matches!(self.public_notes, NullableUpdate::Unchanged)
            || !matches!(self.internal_notes, NullableUpdate::Unchanged)
    }
}

pub fn nullable_text(value: Option<Option<String>>) -> NullableUpdate<String> {
    match value {
        Some(Some(value)) => match normalize_optional_text(Some(value)) {
            Some(value) => NullableUpdate::Set(value),
            None => NullableUpdate::Clear,
        },
        Some(None) => NullableUpdate::Clear,
        None => NullableUpdate::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateInventoryItem;
    use crate::domain::NullableUpdate;

    #[test]
    fn nullable_fields_preserve_unchanged_set_and_clear() {
        let update = UpdateInventoryItem::parse(
            None,
            Some(Some(" batch ".into())),
            None,
            None,
            None,
            None,
            None,
            Some(None),
            None,
        )
        .unwrap();
        assert!(matches!(update.batch_number, NullableUpdate::Set(ref value) if value == "batch"));
        assert!(matches!(update.public_notes, NullableUpdate::Clear));
        assert!(matches!(update.internal_notes, NullableUpdate::Unchanged));
    }
}
