#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttachmentStorageBackend {
    Local,
}

impl AttachmentStorageBackend {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "local" => Ok(Self::Local),
            _ => Err(format!(
                "{s} is not a supported attachment storage backend."
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
        }
    }
}

impl std::fmt::Display for AttachmentStorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
