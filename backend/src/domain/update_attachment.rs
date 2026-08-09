use crate::domain::{AttachmentDisplayName, NullableUpdate};

#[derive(Debug)]
pub struct UpdateAttachment {
    pub display_name: Option<AttachmentDisplayName>,
    pub description: NullableUpdate<String>,
    pub is_public: Option<bool>,
}

impl UpdateAttachment {
    pub fn new(
        display_name: Option<AttachmentDisplayName>,
        description: NullableUpdate<String>,
        is_public: Option<bool>,
    ) -> Self {
        Self {
            display_name,
            description,
            is_public,
        }
    }
}
