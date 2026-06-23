use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetParameterCode(String);

impl AssetParameterCode {
    pub fn parse(s: String) -> Result<Self, String> {
        let code = s.trim();
        let mut chars = code.chars();
        let Some(first) = chars.next() else {
            return Err("code is required".into());
        };

        if !first.is_ascii_lowercase() {
            return Err(format!("{s} is not a valid asset parameter code."));
        }
        if code.len() > 64
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("{s} is not a valid asset parameter code."));
        }

        Ok(Self(code.to_string()))
    }
}

impl AsRef<str> for AssetParameterCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AssetParameterCode {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AssetParameterCode> for String {
    fn from(code: AssetParameterCode) -> Self {
        code.0
    }
}
