use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentStorageKey(String);

impl AttachmentStorageKey {
    pub fn parse(s: String) -> Result<Self, String> {
        let value = s.trim();
        let valid = !value.is_empty()
            && value.len() <= 512
            && value.split('/').all(|segment| {
                !segment.is_empty()
                    && segment != "."
                    && segment != ".."
                    && !segment.contains('\\')
                    && !segment.chars().any(char::is_control)
            });
        if valid {
            Ok(Self(value.to_string()))
        } else {
            Err("attachment storage key is invalid".into())
        }
    }
}

impl AsRef<str> for AttachmentStorageKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttachmentStorageKey {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentStorageKey> for String {
    fn from(storage_key: AttachmentStorageKey) -> Self {
        storage_key.0
    }
}
