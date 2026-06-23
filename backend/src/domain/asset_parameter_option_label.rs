use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetParameterOptionLabel(String);

impl AssetParameterOptionLabel {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let is_empty = trimmed.is_empty();
        let is_too_long = trimmed.graphemes(true).count() > 256;

        if is_empty || is_too_long {
            Err(format!("{s} is not a valid asset parameter option label."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for AssetParameterOptionLabel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AssetParameterOptionLabel {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetParameterOptionLabel> for String {
    fn from(label: AssetParameterOptionLabel) -> Self {
        label.0
    }
}
