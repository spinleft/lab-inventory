use super::local::LocalFileStorage;
use crate::configuration::FileStorageSettings;
use crate::domain::{FileName, FileStorageKey, LaboratoryId, StorageBackend, StoredFile};
use anyhow::anyhow;

#[derive(Clone, Debug)]
pub struct FileStorage {
    backend: StorageBackend,
    local: LocalFileStorage,
    max_file_size_bytes: u64,
    upload_token_ttl_minutes: u64,
}

impl FileStorage {
    pub fn new(settings: FileStorageSettings) -> Result<Self, anyhow::Error> {
        let backend = StorageBackend::parse(&settings.backend).map_err(|e| anyhow!(e))?;
        if settings.max_file_size_bytes == 0 {
            return Err(anyhow!("file_storage.max_file_size_bytes must be positive"));
        }
        if settings.upload_token_ttl_minutes == 0 {
            return Err(anyhow!(
                "file_storage.upload_token_ttl_minutes must be positive"
            ));
        }

        Ok(Self {
            backend,
            local: LocalFileStorage::new(settings.local_root),
            max_file_size_bytes: settings.max_file_size_bytes,
            upload_token_ttl_minutes: settings.upload_token_ttl_minutes,
        })
    }

    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_bytes
    }

    pub fn upload_token_ttl_minutes(&self) -> u64 {
        self.upload_token_ttl_minutes
    }

    pub async fn store_upload(
        &self,
        laboratory_id: LaboratoryId,
        original_file_name: &FileName,
        bytes: &[u8],
    ) -> Result<StoredFile, anyhow::Error> {
        if bytes.is_empty() {
            return Err(anyhow!("File uploads cannot be empty"));
        }
        if bytes.len() as u64 > self.max_file_size_bytes {
            return Err(anyhow!("File upload exceeds configured size limit"));
        }

        match self.backend {
            StorageBackend::Local => {
                self.local
                    .store_upload(laboratory_id, original_file_name, bytes)
                    .await
            }
        }
    }

    pub async fn read(&self, storage_key: &FileStorageKey) -> Result<Vec<u8>, anyhow::Error> {
        match self.backend {
            StorageBackend::Local => self.local.read(storage_key).await,
        }
    }

    pub async fn delete(&self, storage_key: &FileStorageKey) -> Result<(), anyhow::Error> {
        match self.backend {
            StorageBackend::Local => self.local.delete(storage_key).await,
        }
    }
}
