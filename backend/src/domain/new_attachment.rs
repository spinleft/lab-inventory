use crate::domain::{AttachmentDisplayName, FileUploadId};

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
