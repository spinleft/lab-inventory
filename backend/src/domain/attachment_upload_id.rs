use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttachmentUploadId(Uuid);

impl AttachmentUploadId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for AttachmentUploadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for AttachmentUploadId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for AttachmentUploadId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentUploadId> for Uuid {
    fn from(upload_id: AttachmentUploadId) -> Self {
        upload_id.0
    }
}
