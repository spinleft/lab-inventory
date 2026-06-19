use std::ops::Deref;
use uuid::Uuid;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct LaboratoryId(Uuid);

impl LaboratoryId {
    pub fn parse(id: Uuid) -> Result<LaboratoryId, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for LaboratoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for LaboratoryId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for LaboratoryId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LaboratoryId> for Uuid {
    fn from(lab_id: LaboratoryId) -> Self {
        lab_id.0
    }
}
