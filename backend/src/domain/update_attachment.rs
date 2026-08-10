use crate::domain::{AttachmentDisplayName, NullableUpdate};

#[derive(Debug)]
pub struct UpdateAttachment {
    pub display_name: Option<AttachmentDisplayName>,
    pub description: NullableUpdate<String>,
    pub is_public: Option<bool>,
}
