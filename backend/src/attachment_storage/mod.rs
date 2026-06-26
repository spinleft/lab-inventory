mod local;
mod model;

pub use model::StoredFile;

use crate::configuration::AttachmentStorageSettings;
use crate::domain::{AttachmentFileName, AttachmentStorageBackend, AttachmentStorageKey};
use anyhow::anyhow;
use local::LocalAttachmentStorage;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AttachmentStorage {
    backend: AttachmentStorageBackend,
    local: LocalAttachmentStorage,
    max_file_size_bytes: u64,
    upload_token_ttl_minutes: i64,
}

impl AttachmentStorage {
    pub fn new(settings: AttachmentStorageSettings) -> Result<Self, anyhow::Error> {
        let backend = AttachmentStorageBackend::parse(&settings.backend).map_err(|e| anyhow!(e))?;
        if settings.max_file_size_bytes == 0 {
            return Err(anyhow!(
                "attachment_storage.max_file_size_bytes must be positive"
            ));
        }
        if settings.upload_token_ttl_minutes <= 0 {
            return Err(anyhow!(
                "attachment_storage.upload_token_ttl_minutes must be positive"
            ));
        }

        Ok(Self {
            backend,
            local: LocalAttachmentStorage::new(settings.local_root),
            max_file_size_bytes: settings.max_file_size_bytes,
            upload_token_ttl_minutes: settings.upload_token_ttl_minutes,
        })
    }

    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_bytes
    }

    pub fn upload_token_ttl_minutes(&self) -> i64 {
        self.upload_token_ttl_minutes
    }

    pub async fn store_upload(
        &self,
        laboratory_id: Uuid,
        original_file_name: &AttachmentFileName,
        bytes: &[u8],
    ) -> Result<StoredFile, anyhow::Error> {
        if bytes.is_empty() {
            return Err(anyhow!("Attachment files cannot be empty"));
        }
        if bytes.len() as u64 > self.max_file_size_bytes {
            return Err(anyhow!("Attachment file exceeds configured size limit"));
        }

        match self.backend {
            AttachmentStorageBackend::Local => {
                self.local
                    .store_upload(laboratory_id, original_file_name, bytes)
                    .await
            }
        }
    }

    pub async fn read(&self, storage_key: &AttachmentStorageKey) -> Result<Vec<u8>, anyhow::Error> {
        match self.backend {
            AttachmentStorageBackend::Local => self.local.read(storage_key).await,
        }
    }

    pub async fn delete(&self, storage_key: &AttachmentStorageKey) -> Result<(), anyhow::Error> {
        match self.backend {
            AttachmentStorageBackend::Local => self.local.delete(storage_key).await,
        }
    }
}
