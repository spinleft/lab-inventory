use crate::domain::LaboratoryId;
use crate::domain::UserType;

#[derive(Debug)]
pub struct UserRole {
    pub user_type: UserType,
    pub laboratory_id: Option<LaboratoryId>,
}

impl UserRole {
    pub fn new(user_type: UserType, laboratory_id: Option<LaboratoryId>) -> Result<Self, String> {
        if !matches!(user_type, UserType::SuperAdmin | UserType::Root) && laboratory_id.is_none() {
            return Err(format!("Laboratory ID is required for {}.", user_type));
        }

        Ok(Self {
            user_type,
            laboratory_id,
        })
    }
}
