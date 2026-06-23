use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetParameterName(String);

impl AssetParameterName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let is_empty = trimmed.is_empty();
        let is_too_long = trimmed.graphemes(true).count() > 256;
        let forbidden_characters = ['/', '\\', '"', '<', '>', '{', '}'];
        let contains_forbidden_characters =
            trimmed.chars().any(|c| forbidden_characters.contains(&c));

        if is_empty || is_too_long || contains_forbidden_characters {
            Err(format!("{s} is not a valid asset parameter name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for AssetParameterName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AssetParameterName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetParameterName> for String {
    fn from(name: AssetParameterName) -> Self {
        name.0
    }
}
