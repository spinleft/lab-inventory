use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDimension(String);

impl UnitDimension {
    pub fn parse(s: &str) -> Result<Self, String> {
        let dimension = s.trim();
        let mut chars = dimension.chars();
        let Some(first) = chars.next() else {
            return Err("dimension is required".into());
        };

        if !first.is_ascii_lowercase() {
            return Err(format!("{s} is not a valid unit dimension."));
        }
        if dimension.len() > 64
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("{s} is not a valid unit dimension."));
        }

        Ok(Self(dimension.to_string()))
    }
}

impl AsRef<str> for UnitDimension {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for UnitDimension {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for UnitDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<UnitDimension> for String {
    fn from(dimension: UnitDimension) -> Self {
        dimension.0
    }
}

#[cfg(test)]
mod tests {
    use super::UnitDimension;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_dimensions_are_parsed_successfully() {
        for dimension in ["count", "length", "temperature", "luminous_intensity"] {
            assert_ok!(UnitDimension::parse(dimension));
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let dimension = UnitDimension::parse("  length  ").unwrap();
        assert_eq!(dimension.as_ref(), "length");
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        for dimension in ["", "1length", "_length", "Length", "unit dimension"] {
            assert_err!(UnitDimension::parse(dimension));
        }
    }
}
