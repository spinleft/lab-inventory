use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelPrinterName(String);

impl LabelPrinterName {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        let is_empty = trimmed.is_empty();
        let is_too_long = trimmed.graphemes(true).count() > 128;
        let forbidden_characters = ['/', '\\', '"', '<', '>', '{', '}'];
        let contains_forbidden_characters =
            trimmed.chars().any(|c| forbidden_characters.contains(&c));

        if is_empty || is_too_long || contains_forbidden_characters {
            Err(format!("{s} is not a valid label printer name."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for LabelPrinterName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for LabelPrinterName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LabelPrinterName> for String {
    fn from(name: LabelPrinterName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::LabelPrinterName;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_names_are_parsed_successfully() {
        for name in ["前台标签机", "Bench QL-820", "printer-1"] {
            assert_ok!(LabelPrinterName::parse(name.into()));
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let name = LabelPrinterName::parse("  前台标签机  ".into()).expect("name is valid");
        assert_eq!(name.as_ref(), "前台标签机");
    }

    #[test]
    fn empty_or_whitespace_only_names_are_rejected() {
        assert_err!(LabelPrinterName::parse(String::new()));
        assert_err!(LabelPrinterName::parse("   ".into()));
    }

    #[test]
    fn overly_long_names_are_rejected() {
        assert_ok!(LabelPrinterName::parse("a".repeat(128)));
        assert_err!(LabelPrinterName::parse("a".repeat(129)));
    }

    #[test]
    fn names_containing_forbidden_characters_are_rejected() {
        for name in ["a/b", "a\\b", "a\"b", "a<b", "a>b", "a{b", "a}b"] {
            assert_err!(LabelPrinterName::parse(name.into()));
        }
    }
}
