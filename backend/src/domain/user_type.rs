#[derive(Debug, PartialEq, Copy, Clone)]
pub enum UserType {
    Root,
    SuperAdmin,
    LabAdmin,
    User,
    Guest,
}

impl UserType {
    pub fn parse(s: &String) -> Result<UserType, String> {
        match s.as_str() {
            "root" => Ok(UserType::Root),
            "super_admin" => Ok(UserType::SuperAdmin),
            "lab_admin" => Ok(UserType::LabAdmin),
            "user" => Ok(UserType::User),
            "guest" => Ok(UserType::Guest),
            _ => Err(format!("{} is not a valid user type.", s)),
        }
    }
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UserType::Root => "root",
            UserType::SuperAdmin => "super_admin",
            UserType::LabAdmin => "lab_admin",
            UserType::User => "user",
            UserType::Guest => "guest",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::UserType;
    use claims::{assert_err, assert_ok};

    #[test]
    fn valid_user_types_are_parsed_successfully() {
        for user_type in ["root", "super_admin", "lab_admin", "user", "guest"] {
            assert_ok!(UserType::parse(&user_type.into()));
        }
    }

    #[test]
    fn invalid_user_types_are_rejected() {
        for user_type in [
            "",
            "admin",
            "superadmin",
            "labadmin",
            "users",
            "guests",
            "ROOT",
        ] {
            assert_err!(UserType::parse(&user_type.into()));
        }
    }
}
