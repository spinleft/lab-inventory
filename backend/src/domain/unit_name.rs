use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitName(String);

impl UnitName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let is_empty = trimmed.is_empty();
        let is_too_long = trimmed.graphemes(true).count() > 128;
        let forbidden_characters = ['/', '\\', '"', '<', '>', '{', '}'];
        let contains_forbidden_characters =
            trimmed.chars().any(|c| forbidden_characters.contains(&c));

        if is_empty || is_too_long || contains_forbidden_characters {
            Err(format!("{s} is not a valid unit name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for UnitName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for UnitName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<UnitName> for String {
    fn from(name: UnitName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::UnitName;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_names_are_parsed_successfully() {
        assert_ok!(UnitName::parse("Millimeter".into()));
        assert_ok!(UnitName::parse("毫米".into()));
    }

    #[test]
    fn invalid_names_are_rejected() {
        assert_err!(UnitName::parse("".into()));
        assert_err!(UnitName::parse("   ".into()));
        assert_err!(UnitName::parse("a".repeat(129)));
        assert_err!(UnitName::parse("/".into()));
    }
}
