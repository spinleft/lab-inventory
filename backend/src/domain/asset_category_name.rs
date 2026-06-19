use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCategoryName(String);

impl AssetCategoryName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let is_empty = trimmed.is_empty();
        let is_too_long = trimmed.graphemes(true).count() > 256;
        let forbidden_characters = ['/', '\\', '"', '<', '>', '{', '}'];
        let contains_forbidden_characters =
            trimmed.chars().any(|c| forbidden_characters.contains(&c));

        if is_empty || is_too_long || contains_forbidden_characters {
            Err(format!("{s} is not a valid asset category name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for AssetCategoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AssetCategoryName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetCategoryName> for String {
    fn from(name: AssetCategoryName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::AssetCategoryName;
    use claims::{assert_err, assert_ok};

    #[test]
    fn a_valid_name_is_parsed_successfully() {
        assert_ok!(AssetCategoryName::parse("显微镜".into()));
        assert_ok!(AssetCategoryName::parse("Microscopes".into()));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let name = AssetCategoryName::parse("  Microscopes  ".into()).unwrap();
        assert_eq!(name.as_ref(), "Microscopes");
    }

    #[test]
    fn empty_or_whitespace_names_are_rejected() {
        assert_err!(AssetCategoryName::parse("".into()));
        assert_err!(AssetCategoryName::parse("   ".into()));
    }

    #[test]
    fn names_longer_than_256_graphemes_are_rejected() {
        assert_err!(AssetCategoryName::parse("a".repeat(257)));
    }

    #[test]
    fn names_with_forbidden_characters_are_rejected() {
        for name in ["/", "\\", "\"", "<", ">", "{", "}"] {
            assert_err!(AssetCategoryName::parse(name.into()));
        }
    }
}
