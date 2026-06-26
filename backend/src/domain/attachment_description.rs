use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDescription(String);

impl AttachmentDescription {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("attachment description cannot be empty".into());
        }
        if trimmed.graphemes(true).count() > 2000 {
            return Err("attachment description is too long".into());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn parse_optional(s: String) -> Result<Option<Self>, String> {
        if s.trim().is_empty() {
            Ok(None)
        } else {
            Self::parse(s).map(Some)
        }
    }
}

impl AsRef<str> for AttachmentDescription {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttachmentDescription {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentDescription> for String {
    fn from(description: AttachmentDescription) -> Self {
        description.0
    }
}
