use crate::domain::{
    AssetTrackingMode, InventoryItemSerialNumber, InventoryStatus, LocationId,
};
use std::collections::HashSet;

/// Quantities are always expressed in the owning asset's `inventory_unit_id`, so
/// an inventory item never carries a unit of its own.
#[derive(Clone, Debug)]
pub struct NewInventoryItem {
    pub serial_number: Option<InventoryItemSerialNumber>,
    pub batch_number: Option<String>,
    pub quantity_on_hand: f64,
    pub quantity_allocated: f64,
    pub location_id: Option<LocationId>,
    pub status: InventoryStatus,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

impl NewInventoryItem {
    pub fn serialized(
        serial_number: InventoryItemSerialNumber,
        batch_number: Option<String>,
        location_id: Option<LocationId>,
        status: InventoryStatus,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Self {
        Self {
            serial_number: Some(serial_number),
            batch_number: normalize_optional_text(batch_number),
            quantity_on_hand: 1.0,
            quantity_allocated: 0.0,
            location_id,
            status,
            public_notes: normalize_optional_text(public_notes),
            internal_notes: normalize_optional_text(internal_notes),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn quantity(
        batch_number: Option<String>,
        quantity_on_hand: f64,
        quantity_allocated: f64,
        location_id: Option<LocationId>,
        status: InventoryStatus,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Result<Self, String> {
        validate_quantities(quantity_on_hand, quantity_allocated)?;
        Ok(Self {
            serial_number: None,
            batch_number: normalize_optional_text(batch_number),
            quantity_on_hand,
            quantity_allocated,
            location_id,
            status,
            public_notes: normalize_optional_text(public_notes),
            internal_notes: normalize_optional_text(internal_notes),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn parse_for_tracking_mode(
        tracking_mode: AssetTrackingMode,
        serial_number: Option<String>,
        batch_number: Option<String>,
        quantity_on_hand: Option<f64>,
        quantity_allocated: Option<f64>,
        location_id: Option<LocationId>,
        status: Option<String>,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Result<Self, String> {
        let status = status
            .as_deref()
            .map(InventoryStatus::parse)
            .transpose()?
            .unwrap_or(InventoryStatus::Available);
        match tracking_mode {
            AssetTrackingMode::Serialized => {
                if quantity_on_hand.is_some() || quantity_allocated.is_some() {
                    return Err("Serialized inventory items cannot specify quantity fields".into());
                }
                Ok(Self::serialized(
                    InventoryItemSerialNumber::parse(serial_number.ok_or_else(|| {
                        "Serialized inventory items require serial_number".to_string()
                    })?)?,
                    batch_number,
                    location_id,
                    status,
                    public_notes,
                    internal_notes,
                ))
            }
            AssetTrackingMode::Quantity => {
                if serial_number.is_some() {
                    return Err(
                        "Quantity-tracked inventory items cannot specify serial_number".into(),
                    );
                }
                Self::quantity(
                    batch_number,
                    quantity_on_hand.ok_or_else(|| {
                        "Quantity-tracked inventory items require quantity_on_hand".to_string()
                    })?,
                    quantity_allocated.unwrap_or(0.0),
                    location_id,
                    status,
                    public_notes,
                    internal_notes,
                )
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum NewInventoryItems {
    Serialized {
        serial_source: InventoryItemSerialSource,
        batch_number: Option<String>,
        location_id: Option<LocationId>,
        status: InventoryStatus,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    },
    Quantity(NewInventoryItem),
}

impl NewInventoryItems {
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        tracking_mode: AssetTrackingMode,
        serial_items: Option<Vec<String>>,
        serial_numbers: Option<Vec<String>>,
        count: Option<i64>,
        batch_number: Option<String>,
        quantity_on_hand: Option<f64>,
        quantity_allocated: Option<f64>,
        location_id: Option<LocationId>,
        status: Option<String>,
        public_notes: Option<String>,
        internal_notes: Option<String>,
    ) -> Result<Self, String> {
        let status = status
            .as_deref()
            .map(InventoryStatus::parse)
            .transpose()?
            .unwrap_or(InventoryStatus::Available);
        match tracking_mode {
            AssetTrackingMode::Serialized => {
                if quantity_on_hand.is_some() || quantity_allocated.is_some() {
                    return Err("Serialized inventory items cannot specify quantity fields".into());
                }
                Ok(Self::Serialized {
                    serial_source: InventoryItemSerialSource::parse(
                        serial_items,
                        serial_numbers,
                        count,
                    )?,
                    batch_number: normalize_optional_text(batch_number),
                    location_id,
                    status,
                    public_notes: normalize_optional_text(public_notes),
                    internal_notes: normalize_optional_text(internal_notes),
                })
            }
            AssetTrackingMode::Quantity => {
                if serial_items.is_some() || serial_numbers.is_some() || count.is_some() {
                    return Err("Quantity-tracked inventory items cannot specify serial_items, serial_numbers, or count".into());
                }
                Ok(Self::Quantity(NewInventoryItem::quantity(
                    batch_number,
                    quantity_on_hand.ok_or_else(|| {
                        "Quantity-tracked inventory items require quantity_on_hand".to_string()
                    })?,
                    quantity_allocated.unwrap_or(0.0),
                    location_id,
                    status,
                    public_notes,
                    internal_notes,
                )?))
            }
        }
    }

    pub fn location_id(&self) -> Option<LocationId> {
        match self {
            Self::Serialized { location_id, .. } => *location_id,
            Self::Quantity(item) => item.location_id,
        }
    }
}

#[derive(Clone, Debug)]
pub enum InventoryItemSerialSource {
    Explicit(Vec<InventoryItemSerialNumber>),
    Generate(u16),
}

impl InventoryItemSerialSource {
    pub fn parse(
        serial_items: Option<Vec<String>>,
        serial_numbers: Option<Vec<String>>,
        count: Option<i64>,
    ) -> Result<Self, String> {
        match (serial_items, serial_numbers, count) {
            (Some(items), None, None) | (None, Some(items), None) => Self::explicit(items),
            (None, None, Some(count)) => Self::generate(count),
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) | (None, Some(_), Some(_)) => Err(
                "serialized creation accepts serial_items, serial_numbers, or count, not more than one"
                    .into(),
            ),
            (None, None, None) => Err(
                "serialized creation requires serial_items, serial_numbers, or count".into(),
            ),
        }
    }

    pub fn explicit(values: Vec<String>) -> Result<Self, String> {
        if values.is_empty() {
            return Err("serial_numbers cannot be empty".into());
        }
        if values.len() > 200 {
            return Err("serial_numbers cannot contain more than 200 values".into());
        }

        let parsed = values
            .into_iter()
            .map(InventoryItemSerialNumber::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = HashSet::with_capacity(parsed.len());
        if parsed
            .iter()
            .any(|serial_number| !seen.insert(serial_number.as_ref().to_string()))
        {
            return Err("serial_numbers cannot contain duplicates".into());
        }
        Ok(Self::Explicit(parsed))
    }

    pub fn generate(count: i64) -> Result<Self, String> {
        if count <= 0 {
            return Err("count must be positive".into());
        }
        if count > 200 {
            return Err("count cannot exceed 200".into());
        }
        Ok(Self::Generate(count as u16))
    }
}

pub fn validate_quantities(quantity_on_hand: f64, quantity_allocated: f64) -> Result<(), String> {
    if !quantity_on_hand.is_finite() {
        return Err("quantity_on_hand must be finite".into());
    }
    if !quantity_allocated.is_finite() {
        return Err("quantity_allocated must be finite".into());
    }
    if quantity_on_hand < 0.0 {
        return Err("quantity_on_hand must be non-negative".into());
    }
    if quantity_allocated < 0.0 {
        return Err("quantity_allocated must be non-negative".into());
    }
    if quantity_allocated > quantity_on_hand {
        return Err("quantity_allocated cannot exceed quantity_on_hand".into());
    }
    Ok(())
}

pub fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        InventoryItemSerialSource, NewInventoryItem, NewInventoryItems, validate_quantities,
    };
    use crate::domain::AssetTrackingMode;

    #[test]
    fn serial_sources_reject_empty_duplicate_and_oversized_inputs() {
        assert!(InventoryItemSerialSource::explicit(Vec::new()).is_err());
        assert!(InventoryItemSerialSource::explicit(vec!["SN-1".into(), " SN-1 ".into()]).is_err());
        assert!(InventoryItemSerialSource::explicit(vec!["SN".into(); 201]).is_err());
        assert!(InventoryItemSerialSource::generate(0).is_err());
        assert!(InventoryItemSerialSource::generate(201).is_err());
    }

    #[test]
    fn quantities_must_be_finite_non_negative_and_consistent() {
        assert!(validate_quantities(10.0, 2.0).is_ok());
        assert!(validate_quantities(f64::NAN, 0.0).is_err());
        assert!(validate_quantities(1.0, f64::INFINITY).is_err());
        assert!(validate_quantities(-1.0, 0.0).is_err());
        assert!(validate_quantities(1.0, 2.0).is_err());
    }

    #[test]
    fn serialized_creation_modes_are_mutually_exclusive_and_bounded() {
        assert!(InventoryItemSerialSource::parse(Some(vec!["A".into()]), None, None).is_ok());
        assert!(InventoryItemSerialSource::parse(None, None, Some(200)).is_ok());
        assert!(
            InventoryItemSerialSource::parse(Some(vec!["A".into()]), Some(vec!["B".into()]), None)
                .is_err()
        );
        assert!(InventoryItemSerialSource::parse(None, None, None).is_err());
        assert!(InventoryItemSerialSource::parse(None, None, Some(201)).is_err());
    }

    #[test]
    fn tracking_mode_rejects_incompatible_field_combinations() {
        assert!(
            NewInventoryItem::parse_for_tracking_mode(
                AssetTrackingMode::Serialized,
                Some("SN-1".into()),
                None,
                Some(1.0),
                None,
                None,
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            NewInventoryItems::parse(
                AssetTrackingMode::Quantity,
                None,
                Some(vec!["SN-1".into()]),
                None,
                None,
                Some(1.0),
                None,
                None,
                None,
                None,
                None,
            )
            .is_err()
        );
    }
}
