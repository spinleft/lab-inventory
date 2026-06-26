use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentFileName(String);

impl AttachmentFileName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let invalid = trimmed.is_empty()
            || trimmed.graphemes(true).count() > 255
            || trimmed
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\');
        if invalid {
            Err(format!("{s} is not a valid attachment file name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for AttachmentFileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttachmentFileName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentFileName> for String {
    fn from(file_name: AttachmentFileName) -> Self {
        file_name.0
    }
}
