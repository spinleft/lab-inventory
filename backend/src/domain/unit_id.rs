use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UnitId(pub Uuid);

impl AsRef<Uuid> for UnitId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for UnitId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<UnitId> for Uuid {
    fn from(user_id: UnitId) -> Self {
        user_id.0
    }
}

impl From<Uuid> for UnitId {
    fn from(val: Uuid) -> Self {
        UnitId(val)
    }
}

impl std::fmt::Display for UnitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
