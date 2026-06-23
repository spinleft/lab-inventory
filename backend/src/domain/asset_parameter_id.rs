use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetParameterId(Uuid);

impl AssetParameterId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for AssetParameterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for AssetParameterId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for AssetParameterId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetParameterId> for Uuid {
    fn from(parameter_id: AssetParameterId) -> Self {
        parameter_id.0
    }
}
