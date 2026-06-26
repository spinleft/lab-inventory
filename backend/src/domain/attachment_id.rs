use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttachmentId(Uuid);

impl AttachmentId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for AttachmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for AttachmentId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for AttachmentId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentId> for Uuid {
    fn from(attachment_id: AttachmentId) -> Self {
        attachment_id.0
    }
}
