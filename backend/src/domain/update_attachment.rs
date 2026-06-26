use crate::domain::{
    AttachmentDescription, AttachmentDisplayName, AttachmentVisibility, NullableUpdate,
};

#[derive(Debug)]
pub struct UpdateAttachment {
    pub display_name: Option<AttachmentDisplayName>,
    pub description: NullableUpdate<AttachmentDescription>,
    pub visibility: Option<AttachmentVisibility>,
}

impl UpdateAttachment {
    pub fn new(
        display_name: Option<AttachmentDisplayName>,
        description: NullableUpdate<AttachmentDescription>,
        visibility: Option<AttachmentVisibility>,
    ) -> Self {
        Self {
            display_name,
            description,
            visibility,
        }
    }
}
