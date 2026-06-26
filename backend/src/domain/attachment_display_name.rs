use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDisplayName(String);

impl AttachmentDisplayName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let forbidden_characters = ['/', '\\', '"', '<', '>', '{', '}'];
        let invalid = trimmed.is_empty()
            || trimmed.graphemes(true).count() > 256
            || trimmed
                .chars()
                .any(|c| c.is_control() || forbidden_characters.contains(&c));
        if invalid {
            Err(format!("{s} is not a valid attachment display name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for AttachmentDisplayName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttachmentDisplayName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentDisplayName> for String {
    fn from(display_name: AttachmentDisplayName) -> Self {
        display_name.0
    }
}
