use crate::domain::{
    AttachmentFileSize, AttachmentSha256, AttachmentStorageBackend, AttachmentStorageKey,
};

pub struct StoredFile {
    pub storage_backend: AttachmentStorageBackend,
    pub storage_key: AttachmentStorageKey,
    pub file_size_bytes: AttachmentFileSize,
    pub sha256_hex: AttachmentSha256,
}
