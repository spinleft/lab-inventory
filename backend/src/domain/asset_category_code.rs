use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCategoryCode(String);

impl AssetCategoryCode {
    pub fn parse(s: String) -> Result<Self, String> {
        let code = s.trim();
        let mut chars = code.chars();
        let Some(first) = chars.next() else {
            return Err("code is required".into());
        };

        if !first.is_ascii_lowercase() {
            return Err(format!("{s} is not a valid asset category code."));
        }
        if code.len() > 64
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("{s} is not a valid asset category code."));
        }

        Ok(Self(code.to_string()))
    }
}

impl AsRef<str> for AssetCategoryCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AssetCategoryCode {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetCategoryCode> for String {
    fn from(code: AssetCategoryCode) -> Self {
        code.0
    }
}

#[cfg(test)]
mod tests {
    use super::AssetCategoryCode;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_codes_are_parsed_successfully() {
        for code in ["equipment", "equipment_2", "x", "a123"] {
            assert_ok!(AssetCategoryCode::parse(code.into()));
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let code = AssetCategoryCode::parse("  microscope  ".into()).unwrap();
        assert_eq!(code.as_ref(), "microscope");
    }

    #[test]
    fn codes_must_start_with_a_lowercase_ascii_letter() {
        for code in ["", "1equipment", "_equipment", "Equipment"] {
            assert_err!(AssetCategoryCode::parse(code.into()));
        }
    }

    #[test]
    fn codes_may_only_contain_lowercase_ascii_digits_and_underscores() {
        for code in ["equipment-name", "equipment.name", "设备", "equipment name"] {
            assert_err!(AssetCategoryCode::parse(code.into()));
        }
    }

    #[test]
    fn codes_longer_than_64_bytes_are_rejected() {
        assert_err!(AssetCategoryCode::parse(format!("a{}", "b".repeat(64))));
    }
}
