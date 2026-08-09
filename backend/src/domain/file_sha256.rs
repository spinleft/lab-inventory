use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSha256(String);

impl FileSha256 {
    pub fn parse(s: String) -> Result<Self, String> {
        let value = s.trim();
        if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err("attachment sha256 must be a 64-character hex digest".into())
        }
    }
}

impl AsRef<str> for FileSha256 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for FileSha256 {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<FileSha256> for String {
    fn from(sha256: FileSha256) -> Self {
        sha256.0
    }
}
