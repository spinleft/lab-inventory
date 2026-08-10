use rand::Rng;
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, Secret};

#[derive(Debug)]
pub struct GuestRegistrationCode(Secret<String>);

impl GuestRegistrationCode {
    pub fn parse(value: Secret<String>) -> Result<Self, String> {
        let value = value.expose_secret();
        if value.len() == 6 && value.chars().all(|character| character.is_ascii_digit()) {
            Ok(Self(Secret::new(value.clone())))
        } else {
            Err("Registration code must contain exactly 6 digits.".into())
        }
    }

    pub fn generate() -> Self {
        let value = OsRng.gen_range(0..1_000_000);
        Self(Secret::new(format!("{value:06}")))
    }
}

impl AsRef<Secret<String>> for GuestRegistrationCode {
    fn as_ref(&self) -> &Secret<String> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::GuestRegistrationCode;
    use claims::{assert_err, assert_ok};
    use secrecy::{ExposeSecret, Secret};

    #[test]
    fn six_digit_codes_are_accepted_including_leading_zeroes() {
        for code in ["000000", "012345", "999999"] {
            assert_ok!(GuestRegistrationCode::parse(Secret::new(code.into())));
        }
    }

    #[test]
    fn malformed_codes_are_rejected() {
        for code in ["", "12345", "1234567", "12345a", "１２３４５６"] {
            assert_err!(GuestRegistrationCode::parse(Secret::new(code.into())));
        }
    }

    #[test]
    fn generated_codes_always_have_six_ascii_digits() {
        for _ in 0..100 {
            let code = GuestRegistrationCode::generate();
            let value = code.as_ref().expose_secret();
            assert_eq!(value.len(), 6);
            assert!(value.chars().all(|character| character.is_ascii_digit()));
        }
    }
}
