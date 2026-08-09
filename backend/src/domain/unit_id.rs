use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UnitId(pub Uuid);

impl From<UnitId> for Uuid {
    fn from(user_id: UnitId) -> Self {
        user_id.0
    }
}

impl Into<UnitId> for Uuid {
    fn into(self) -> UnitId {
        UnitId(self)
    }
}

impl std::fmt::Display for UnitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
