use std::ops::Deref;

use secrecy::{ExposeSecret, Secret};

#[derive(Debug, Clone)]
pub struct UserPassword(pub Secret<String>);

impl UserPassword {
    pub fn parse(s: Secret<String>) -> Result<UserPassword, String> {
        // Password validation rules can be added here, e.g., minimum length, complexity, etc.
        // Passwords must be at least 8 characters long
        if validate_password(&s) {
            Ok(Self(s))
        } else {
            Err("Password must be at least 8 characters long.".to_string())
        }
    }
}

impl AsRef<Secret<String>> for UserPassword {
    fn as_ref(&self) -> &Secret<String> {
        &self.0
    }
}

impl Deref for UserPassword {
    type Target = Secret<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn validate_password(password: &Secret<String>) -> bool {
    if password.expose_secret().trim().len() < 8 {
        false
    } else {
        true
    }
}
