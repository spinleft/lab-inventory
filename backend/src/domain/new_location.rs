use crate::domain::{LocationCode, LocationId, LocationName};

#[derive(Debug)]
pub struct NewLocation {
    pub parent_location_id: Option<LocationId>,
    pub name: LocationName,
    pub code: LocationCode,
    pub description: Option<String>,
}

impl NewLocation {
    pub fn new(
        parent_location_id: Option<LocationId>,
        name: LocationName,
        code: LocationCode,
        description: Option<String>,
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
    use super::NewLocation;
    use crate::domain::{LocationCode, LocationId, LocationName};
    use uuid::Uuid;

    #[test]
    fn new_location_keeps_parent_name_code_and_description() {
        let parent_location_id = LocationId::parse(Uuid::new_v4()).unwrap();
        let name = LocationName::parse("Room A".into()).unwrap();
        let code = LocationCode::parse("room_a".into()).unwrap();

        let location = NewLocation::new(
            Some(parent_location_id),
            name,
            code,
            Some("Main lab room".into()),
        );

        assert_eq!(location.parent_location_id, Some(parent_location_id));
        assert_eq!(location.name.as_ref(), "Room A");
        assert_eq!(location.code.as_ref(), "room_a");
        assert_eq!(location.description.as_deref(), Some("Main lab room"));
    }
}
