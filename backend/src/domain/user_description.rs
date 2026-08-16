use std::ops::Deref;
use unicode_segmentation::UnicodeSegmentation;

/// A free-text note on an account: who this is, why they have access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDescription(String);

impl UserDescription {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("user description cannot be empty".into());
        }
        if trimmed.graphemes(true).count() > 500 {
            return Err("user description is too long".into());
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Treats blank input as "no note", so a form that submits an empty field
    /// clears the column rather than storing a string of spaces.
    pub fn parse_optional(s: String) -> Result<Option<Self>, String> {
        if s.trim().is_empty() {
            Ok(None)
        } else {
            Self::parse(s).map(Some)
        }
    }
}

impl AsRef<str> for UserDescription {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for UserDescription {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<UserDescription> for String {
    fn from(description: UserDescription) -> Self {
        description.0
    }
}

#[cfg(test)]
mod tests {
    use super::UserDescription;
    use claims::{assert_err, assert_ok};

    #[test]
    fn surrounding_whitespace_is_dropped() {
        let description = UserDescription::parse("  仪器室钥匙管理员  ".into()).unwrap();
        assert_eq!(description.as_ref(), "仪器室钥匙管理员");
    }

    #[test]
    fn blank_input_is_no_description_rather_than_an_error() {
        assert_eq!(UserDescription::parse_optional("   ".into()), Ok(None));
        assert_err!(UserDescription::parse("   ".into()));
    }

    #[test]
    fn the_limit_counts_graphemes_not_bytes() {
        assert_ok!(UserDescription::parse("实".repeat(500)));
        assert_err!(UserDescription::parse("实".repeat(501)));
    }
}
