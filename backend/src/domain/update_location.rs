use crate::domain::{LocationCode, LocationId, LocationName, NullableUpdate};

#[derive(Debug)]
pub struct UpdateLocation {
    pub parent_location_id: NullableUpdate<LocationId>,
    pub name: Option<LocationName>,
    pub code: Option<LocationCode>,
    pub description: NullableUpdate<String>,
}

impl UpdateLocation {
    pub fn new(
        parent_location_id: NullableUpdate<LocationId>,
        name: Option<LocationName>,
        code: Option<LocationCode>,
        description: NullableUpdate<String>,
    ) -> Self {
        Self {
            parent_location_id,
            name,
            code,
            description,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateLocation;
    use crate::domain::{LocationCode, LocationId, LocationName, NullableUpdate};
    use uuid::Uuid;

    #[test]
    fn update_location_captures_partial_updates() {
        let parent_location_id = LocationId::parse(Uuid::new_v4()).unwrap();
        let name = LocationName::parse("Room A".into()).unwrap();
        let code = LocationCode::parse("room_a".into()).unwrap();

        let update = UpdateLocation::new(
            NullableUpdate::Set(parent_location_id),
            Some(name),
            Some(code),
            NullableUpdate::Clear,
        );

        assert!(matches!(
            update.parent_location_id,
            NullableUpdate::Set(value) if value == parent_location_id
        ));
        assert_eq!(
            update.name.as_ref().map(|name| name.as_ref()),
            Some("Room A")
        );
        assert_eq!(
            update.code.as_ref().map(|code| code.as_ref()),
            Some("room_a")
        );
        assert!(matches!(update.description, NullableUpdate::Clear));
    }
}
