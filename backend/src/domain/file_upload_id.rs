use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileUploadId(pub Uuid);

impl FileUploadId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for FileUploadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for FileUploadId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for FileUploadId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<FileUploadId> for Uuid {
    fn from(upload_id: FileUploadId) -> Self {
        upload_id.0
    }
}

impl Into<FileUploadId> for Uuid {
    fn into(self) -> FileUploadId {
        FileUploadId(self)
    }
}
