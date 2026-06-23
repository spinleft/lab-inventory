use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitCode(String);

impl UnitCode {
    pub fn parse(s: String) -> Result<Self, String> {
        let code = s.trim();
        let mut chars = code.chars();
        let Some(first) = chars.next() else {
            return Err("code is required".into());
        };

        if !first.is_ascii_lowercase() {
            return Err(format!("{s} is not a valid unit code."));
        }
        if code.len() > 64
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("{s} is not a valid unit code."));
        }

        Ok(Self(code.to_string()))
    }
}

impl AsRef<str> for UnitCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for UnitCode {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<UnitCode> for String {
    fn from(code: UnitCode) -> Self {
        code.0
    }
}

#[cfg(test)]
mod tests {
    use super::UnitCode;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_codes_are_parsed_successfully() {
        for code in ["m", "mm", "inch", "pcs", "unit_2"] {
            assert_ok!(UnitCode::parse(code.into()));
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let code = UnitCode::parse("  mm  ".into()).unwrap();
        assert_eq!(code.as_ref(), "mm");
    }

    #[test]
    fn invalid_codes_are_rejected() {
        for code in ["", "1mm", "_mm", "MM", "m/s", "unit code"] {
            assert_err!(UnitCode::parse(code.into()));
        }
    }
}
