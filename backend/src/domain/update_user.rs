use crate::domain::LaboratoryId;
use crate::domain::PhoneNumber;
use crate::domain::UserEmail;
use crate::domain::UserName;
use crate::domain::UserType;

#[derive(Debug)]
pub enum NullableUpdate<T> {
    Unchanged,
    Set(T),
    Clear,
}

impl<T> NullableUpdate<T> {
    pub fn is_changed(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub fn resolve(self, current: Option<T>) -> Option<T> {
        match self {
            Self::Unchanged => current,
            Self::Set(value) => Some(value),
            Self::Clear => None,
        }
    }
}

#[derive(Debug)]
pub struct UpdateUser {
    pub username: Option<UserName>,
    pub user_type: UserType,
    user_type_changed: bool,
    pub laboratory_id: Option<LaboratoryId>,
    laboratory_id_changed: bool,
    pub email: NullableUpdate<UserEmail>,
    pub phone_number: NullableUpdate<PhoneNumber>,
}

impl UpdateUser {
    pub fn new(
        username: Option<UserName>,
        user_type: Option<UserType>,
        laboratory_id: NullableUpdate<LaboratoryId>,
        email: NullableUpdate<UserEmail>,
        phone_number: NullableUpdate<PhoneNumber>,
        current_user_type: UserType,
        current_laboratory_id: Option<LaboratoryId>,
    ) -> Result<Self, String> {
        let user_type_changed = user_type.is_some();
        let laboratory_id_changed = laboratory_id.is_changed();
        let user_type = user_type.unwrap_or(current_user_type);
        let laboratory_id = laboratory_id.resolve(current_laboratory_id);

        if !matches!(user_type, UserType::SuperAdmin | UserType::Root) && laboratory_id.is_none() {
            return Err(format!("Laboratory ID is required for {}.", user_type));
        }

        Ok(Self {
            username,
            user_type,
            user_type_changed,
            laboratory_id,
            laboratory_id_changed,
            email,
            phone_number,
        })
    }

    pub fn updates_role_or_laboratory(&self) -> bool {
        self.user_type_changed || self.laboratory_id_changed
    }
}

#[cfg(test)]
mod tests {
    use super::{NullableUpdate, UpdateUser};
    use crate::domain::{LaboratoryId, UserType};
    use claims::{assert_err, assert_ok};
    use uuid::Uuid;

    fn laboratory_id() -> LaboratoryId {
        LaboratoryId::parse(Uuid::new_v4()).unwrap()
    }

    #[test]
    fn scoped_users_cannot_be_updated_without_a_laboratory() {
        for user_type in [UserType::LabAdmin, UserType::User, UserType::Guest] {
            assert_err!(UpdateUser::new(
                None,
                Some(user_type),
                NullableUpdate::Unchanged,
                NullableUpdate::Unchanged,
                NullableUpdate::Unchanged,
                UserType::Root,
                None,
            ));
        }
    }

    #[test]
    fn scoped_users_cannot_clear_their_laboratory() {
        assert_err!(UpdateUser::new(
            None,
            None,
            NullableUpdate::Clear,
            NullableUpdate::Unchanged,
            NullableUpdate::Unchanged,
            UserType::User,
            Some(laboratory_id()),
        ));
    }

    #[test]
    fn server_scoped_users_can_be_updated_without_a_laboratory() {
        for user_type in [UserType::Root, UserType::SuperAdmin] {
            assert_ok!(UpdateUser::new(
                None,
                Some(user_type),
                NullableUpdate::Clear,
                NullableUpdate::Unchanged,
                NullableUpdate::Unchanged,
                UserType::User,
                Some(laboratory_id()),
            ));
        }
    }

    #[test]
    fn scoped_users_can_keep_their_laboratory() {
        assert_ok!(UpdateUser::new(
            None,
            None,
            NullableUpdate::Unchanged,
            NullableUpdate::Unchanged,
            NullableUpdate::Unchanged,
            UserType::User,
            Some(laboratory_id()),
        ));
    }
}
