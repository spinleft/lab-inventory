use crate::domain::{
    AttachmentDescription, AttachmentDisplayName, AttachmentUploadId, AttachmentVisibility,
};

#[derive(Clone, Debug)]
pub struct AttachmentClaim {
    pub upload_id: AttachmentUploadId,
    pub display_name: Option<AttachmentDisplayName>,
    pub description: Option<AttachmentDescription>,
    pub visibility: AttachmentVisibility,
}

impl AttachmentClaim {
    pub fn new(
        upload_id: AttachmentUploadId,
        display_name: Option<AttachmentDisplayName>,
        description: Option<AttachmentDescription>,
        visibility: Option<AttachmentVisibility>,
    ) -> Self {
        Self {
            upload_id,
            display_name,
            description,
            visibility: visibility.unwrap_or_default(),
        }
    }
}
