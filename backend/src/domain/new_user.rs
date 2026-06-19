use crate::domain::LaboratoryId;
use crate::domain::PhoneNumber;
use crate::domain::UserEmail;
use crate::domain::UserName;
use crate::domain::UserPassword;
use crate::domain::UserType;

#[derive(Debug)]
pub struct NewUser {
    pub username: UserName,
    pub password: UserPassword,
    pub user_type: UserType,
    pub laboratory_id: Option<LaboratoryId>,
    pub email: Option<UserEmail>,
    pub phone_number: Option<PhoneNumber>,
}

impl NewUser {
    pub fn new(
        username: UserName,
        password: UserPassword,
        user_type: UserType,
        laboratory_id: Option<LaboratoryId>,
        email: Option<UserEmail>,
        phone_number: Option<PhoneNumber>,
    ) -> Result<Self, String> {
        if !matches!(user_type, UserType::SuperAdmin | UserType::Root) && laboratory_id.is_none() {
            return Err(format!("Laboratory ID is required for {}.", user_type));
        }

        Ok(Self {
            username,
            password,
            user_type,
            laboratory_id,
            email,
            phone_number,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NewUser;
    use crate::domain::{LaboratoryId, UserEmail, UserName, UserPassword, UserType};
    use claims::{assert_err, assert_ok};
    use secrecy::Secret;
    use uuid::Uuid;

    fn valid_username() -> UserName {
        UserName::parse("testuser".into()).unwrap()
    }

    fn valid_password() -> UserPassword {
        UserPassword::parse(Secret::new("password".into())).unwrap()
    }

    fn valid_email() -> UserEmail {
        UserEmail::parse("testuser@example.com".into()).unwrap()
    }

    #[test]
    fn scoped_users_require_a_laboratory() {
        for user_type in [UserType::LabAdmin, UserType::User, UserType::Guest] {
            assert_err!(NewUser::new(
                valid_username(),
                valid_password(),
                user_type,
                None,
                Some(valid_email()),
                None,
            ));
        }
    }

    #[test]
    fn server_scoped_users_can_be_created_without_a_laboratory() {
        for user_type in [UserType::Root, UserType::SuperAdmin] {
            assert_ok!(NewUser::new(
                valid_username(),
                valid_password(),
                user_type,
                None,
                Some(valid_email()),
                None,
            ));
        }
    }

    #[test]
    fn laboratory_scoped_users_can_be_created_with_a_laboratory() {
        assert_ok!(NewUser::new(
            valid_username(),
            valid_password(),
            UserType::LabAdmin,
            Some(LaboratoryId::parse(Uuid::new_v4()).unwrap()),
            Some(valid_email()),
            None,
        ));
    }
}
