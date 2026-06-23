use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSymbol(String);

impl UnitSymbol {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() || trimmed.graphemes(true).count() > 32 {
            Err(format!("{s} is not a valid unit symbol."))
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }
}

impl AsRef<str> for UnitSymbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for UnitSymbol {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<UnitSymbol> for String {
    fn from(symbol: UnitSymbol) -> Self {
        symbol.0
    }
}

#[cfg(test)]
mod tests {
    use super::UnitSymbol;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_symbols_are_parsed_successfully() {
        assert_ok!(UnitSymbol::parse("mm".into()));
        assert_ok!(UnitSymbol::parse("μL".into()));
    }

    #[test]
    fn invalid_symbols_are_rejected() {
        assert_err!(UnitSymbol::parse("".into()));
        assert_err!(UnitSymbol::parse("   ".into()));
        assert_err!(UnitSymbol::parse("a".repeat(33)));
    }
}
