#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssetTrackingMode {
    Serialized,
    Quantity,
}

impl AssetTrackingMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "serialized" => Ok(Self::Serialized),
            "quantity" => Ok(Self::Quantity),
            _ => Err(format!("{s} is not a valid asset tracking mode.")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Serialized => "serialized",
            Self::Quantity => "quantity",
        }
    }
}

impl std::fmt::Display for AssetTrackingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
