use super::model::StoredFile;
use crate::domain::{
    AttachmentFileName, AttachmentFileSize, AttachmentSha256, AttachmentStorageBackend,
    AttachmentStorageKey,
};
use anyhow::{Context, anyhow};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct LocalAttachmentStorage {
    root: Arc<PathBuf>,
}

impl LocalAttachmentStorage {
    pub(super) fn new(root: String) -> Self {
        Self {
            root: Arc::new(PathBuf::from(root)),
        }
    }

    pub(super) async fn store_upload(
        &self,
        laboratory_id: Uuid,
        original_file_name: &AttachmentFileName,
        bytes: &[u8],
    ) -> Result<StoredFile, anyhow::Error> {
        let sha256_hex = AttachmentSha256::parse(sha256_hex(bytes)).map_err(|e| anyhow!(e))?;
        let object_id = Uuid::new_v4();
        let extension = storage_extension(original_file_name.as_ref());
        let file_name = match extension {
            Some(extension) => format!("{}.{extension}", sha256_hex.as_ref()),
            None => sha256_hex.as_ref().to_string(),
        };
        let storage_key = AttachmentStorageKey::parse(format!(
            "labs/{laboratory_id}/objects/{object_id}/{file_name}"
        ))
        .map_err(|e| anyhow!(e))?;
        let path = self.path_for_key(&storage_key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create attachment directory {parent:?}"))?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .with_context(|| format!("Failed to write attachment object {path:?}"))?;

        Ok(StoredFile {
            storage_backend: AttachmentStorageBackend::Local,
            storage_key,
            file_size_bytes: AttachmentFileSize::parse(bytes.len() as i64)
                .map_err(|e| anyhow!(e))?,
            sha256_hex,
        })
    }

    pub(super) async fn read(
        &self,
        storage_key: &AttachmentStorageKey,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let path = self.path_for_key(storage_key)?;
        tokio::fs::read(&path)
            .await
            .with_context(|| format!("Failed to read attachment object {path:?}"))
    }

    pub(super) async fn delete(
        &self,
        storage_key: &AttachmentStorageKey,
    ) -> Result<(), anyhow::Error> {
        let path = self.path_for_key(storage_key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                remove_empty_parent_dir(path.parent()).await;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("Failed to delete attachment object {path:?}"))
            }
        }
    }

    fn path_for_key(&self, storage_key: &AttachmentStorageKey) -> Result<PathBuf, anyhow::Error> {
        let mut path = (*self.root).clone();
        for segment in storage_key.as_ref().split('/') {
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains('\\')
                || segment.contains(std::path::MAIN_SEPARATOR)
            {
                return Err(anyhow!("Invalid attachment storage key"));
            }
            path.push(segment);
        }
        Ok(path)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn storage_extension(file_name: &str) -> Option<String> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())?
        .trim()
        .to_ascii_lowercase();
    if extension.is_empty()
        || extension.len() > 16
        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        None
    } else {
        Some(extension)
    }
}

async fn remove_empty_parent_dir(parent: Option<&Path>) {
    let Some(parent) = parent else {
        return;
    };
    let _ = tokio::fs::remove_dir(parent).await;
}
