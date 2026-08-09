use crate::domain::{FileSha256, FileSize, FileStorageKey, StorageBackend};

pub struct StoredFile {
    pub storage_backend: StorageBackend,
    pub storage_key: FileStorageKey,
    pub file_size_bytes: FileSize,
    pub sha256_hex: FileSha256,
}
