use crate::domain::{AttachmentDisplayName, FileUploadId};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct NewAttachment {
    pub upload_id: FileUploadId,
    pub display_name: Option<AttachmentDisplayName>,
    pub description: Option<String>,
    pub is_public: bool,
}

impl NewAttachment {
    pub fn new(
        upload_id: FileUploadId,
        display_name: Option<AttachmentDisplayName>,
        description: Option<String>,
        is_public: Option<bool>,
    ) -> Self {
        Self {
            upload_id,
            display_name,
            description,
            is_public: is_public.unwrap_or(false),
        }
    }
}

/// An upload can only become one attachment, so it may appear at most once
/// across everything a single create request writes.
pub fn ensure_unique_uploads<'a>(
    attachments: impl IntoIterator<Item = &'a NewAttachment>,
) -> Result<(), String> {
    let mut upload_ids = HashSet::new();
    for attachment in attachments {
        if !upload_ids.insert(attachment.upload_id) {
            return Err("An upload can only be assigned once in a create request".into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NewAttachment, ensure_unique_uploads};
    use uuid::Uuid;

    fn attachment(upload_id: Uuid) -> NewAttachment {
        NewAttachment::new(upload_id.into(), None, None, None)
    }

    #[test]
    fn distinct_uploads_are_accepted() {
        let attachments = vec![attachment(Uuid::new_v4()), attachment(Uuid::new_v4())];

        assert!(ensure_unique_uploads(&attachments).is_ok());
    }

    #[test]
    fn the_same_upload_cannot_be_assigned_twice() {
        let upload_id = Uuid::new_v4();
        let attachments = vec![attachment(upload_id), attachment(upload_id)];

        assert!(ensure_unique_uploads(&attachments).is_err());
    }
}
