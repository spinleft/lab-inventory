use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentSha256(String);

impl AttachmentSha256 {
    pub fn parse(s: String) -> Result<Self, String> {
        let value = s.trim();
        if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err("attachment sha256 must be a 64-character hex digest".into())
        }
    }
}

impl AsRef<str> for AttachmentSha256 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttachmentSha256 {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AttachmentSha256> for String {
    fn from(sha256: AttachmentSha256) -> Self {
        sha256.0
    }
}
