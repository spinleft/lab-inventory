#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssetParameterDataType {
    Text,
    Number,
    Range,
    Boolean,
    Date,
    Enum,
}

impl AssetParameterDataType {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "text" => Ok(Self::Text),
            "number" => Ok(Self::Number),
            "range" => Ok(Self::Range),
            "boolean" => Ok(Self::Boolean),
            "date" => Ok(Self::Date),
            "enum" => Ok(Self::Enum),
            _ => Err(format!("{s} is not a valid asset parameter data type.")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Range => "range",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Enum => "enum",
        }
    }
}

impl std::fmt::Display for AssetParameterDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
