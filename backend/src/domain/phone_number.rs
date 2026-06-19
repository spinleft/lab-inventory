#[derive(Debug)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    pub fn parse(s: String) -> Result<PhoneNumber, String> {
        // Phone number must be 11 digits long and contain only digits
        if s.len() == 11 && s.chars().all(|c| c.is_ascii_digit()) {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid phone number.", s))
        }
    }
}

impl AsRef<str> for PhoneNumber {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PhoneNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<PhoneNumber> for String {
    fn from(phone_number: PhoneNumber) -> Self {
        phone_number.0
    }
}

#[cfg(test)]
mod tests {
    use super::PhoneNumber;
    use claims::{assert_err, assert_ok};

    #[test]
    fn empty_string_is_rejected() {
        let phone_number = "".to_string();
        assert_err!(PhoneNumber::parse(phone_number));
    }

    #[test]
    fn phone_number_with_non_digits_is_rejected() {
        let phone_number = "12345abcde".to_string();
        assert_err!(PhoneNumber::parse(phone_number));
    }

    #[test]
    fn phone_number_with_wrong_length_is_rejected() {
        let phone_number = "123456789".to_string(); // 9 digits instead of 11
        assert_err!(PhoneNumber::parse(phone_number));
    }

    #[test]
    fn valid_phone_number_is_accepted() {
        let phone_number = "12345678901".to_string();
        assert_ok!(PhoneNumber::parse(phone_number));
    }
}
