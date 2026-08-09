#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InventoryStatus {
    Available,
    Reserved,
    Borrowed,
    Retired,
    Lost,
    Consumed,
}

impl InventoryStatus {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "available" => Ok(Self::Available),
            "reserved" => Ok(Self::Reserved),
            "borrowed" => Ok(Self::Borrowed),
            "retired" => Ok(Self::Retired),
            "lost" => Ok(Self::Lost),
            "consumed" => Ok(Self::Consumed),
            _ => Err(format!("{s} is not a valid asset inventory status.")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Reserved => "reserved",
            Self::Borrowed => "borrowed",
            Self::Retired => "retired",
            Self::Lost => "lost",
            Self::Consumed => "consumed",
        }
    }
}

impl std::fmt::Display for InventoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
