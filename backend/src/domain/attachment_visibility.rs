#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttachmentVisibility {
    Public,
    Internal,
}

impl AttachmentVisibility {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "public" => Ok(Self::Public),
            "internal" => Ok(Self::Internal),
            _ => Err(format!("{s} is not a valid attachment visibility.")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
        }
    }
}

impl Default for AttachmentVisibility {
    fn default() -> Self {
        Self::Internal
    }
}

impl std::fmt::Display for AttachmentVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
