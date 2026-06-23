use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocationId(Uuid);

impl LocationId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for LocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for LocationId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for LocationId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LocationId> for Uuid {
    fn from(location_id: LocationId) -> Self {
        location_id.0
    }
}
