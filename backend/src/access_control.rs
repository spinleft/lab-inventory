use crate::domain::LaboratoryId;
use crate::domain::UserId;
use crate::domain::UserType;
use anyhow::Context;
use anyhow::anyhow;
use sqlx::PgPool;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Actor {
    pub user_id: UserId,
    pub user_type: UserType,
    pub laboratory_id: Option<LaboratoryId>,
}

impl Actor {
    pub fn is_guest(&self) -> bool {
        self.user_type == UserType::Guest
    }

    pub fn is_admin(&self) -> bool {
        matches!(
            self.user_type,
            UserType::Root | UserType::SuperAdmin | UserType::LabAdmin
        )
    }

    pub fn is_lab_admin(&self) -> bool {
        self.user_type == UserType::LabAdmin
    }

    pub fn is_super_admin(&self) -> bool {
        self.user_type == UserType::SuperAdmin
    }

    pub fn is_root(&self) -> bool {
        self.user_type == UserType::Root
    }

    pub fn can_manage_user(
        &self,
        target_user_type: UserType,
        target_laboratory_id: Option<LaboratoryId>,
    ) -> bool {
        if !self.is_admin() {
            return false;
        }
        // Laboratory-scoped admins cannot manage root or super_admins
        if self.is_lab_admin() {
            if matches!(target_user_type, UserType::Root | UserType::SuperAdmin) {
                return false;
            } else {
                if let Some(lab_id) = target_laboratory_id {
                    return self.laboratory_id == Some(lab_id);
                } else {
                    // If no lab specified for target, lab admins cannot manage them
                    return false;
                }
            }
        }
        // Super admin cannot manage root users
        if self.is_super_admin() {
            if target_user_type == UserType::Root {
                return false;
            } else {
                return true;
            }
        }
        // Root can manage all users
        if self.is_root() {
            return true;
        }
        false
    }

    pub fn can_view_user(
        &self,
        target_user_id: UserId,
        target_user_type: UserType,
        target_laboratory_id: Option<LaboratoryId>,
    ) -> bool {
        if self.user_id == target_user_id {
            return true;
        }
        if self.is_guest() {
            // Guest users can only view their own information
            return false;
        }
        if self.is_admin() {
            // Lab admins can view users in their lab and all super_admins and guests
            if self.is_lab_admin() {
                if matches!(target_user_type, UserType::SuperAdmin | UserType::Root) {
                    return false;
                } else {
                    if let Some(lab_id) = target_laboratory_id {
                        return self.laboratory_id == Some(lab_id);
                    } else {
                        return false;
                    }
                }
            }
            // Super admin can view all users except root
            if self.is_super_admin() {
                if target_user_type == UserType::Root {
                    return false;
                } else {
                    return true;
                }
            }
            // Root can view all users
            if self.is_root() {
                return true;
            }
        } else {
            // Non-admin users can only view users in their lab
            if let Some(lab_id) = self.laboratory_id {
                return target_laboratory_id == Some(lab_id);
            } else {
                return false;
            }
        }
        false
    }

    pub fn can_view_all_users(&self) -> bool {
        // Only root and super_admin can view all users
        self.is_super_admin() || self.is_root()
    }

    pub fn can_write_laboratory_resource(&self, laboratory_id: LaboratoryId) -> bool {
        // Guest users cannot write
        if self.is_guest() {
            return false;
        }
        self.laboratory_id == Some(laboratory_id) || self.is_admin()
    }
}

pub async fn get_actor(pool: &PgPool, user_id: UserId) -> Result<Option<Actor>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT user_id, user_type_name, laboratory_id
        FROM v_actors
        WHERE user_id = $1
        "#,
        *user_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve actor information")?;

    if let Some(row) = row {
        let user_id = UserId::parse(row.user_id.ok_or(anyhow!("Actor user_id is NULL"))?)
            .map_err(|e| anyhow!("{e}"))?;
        let user_type = UserType::parse(
            &row.user_type_name
                .ok_or(anyhow!("Actor user_type_name is NULL"))?,
        )
        .map_err(|e| anyhow!("{e}"))?;
        let laboratory_id = match row.laboratory_id {
            Some(lab_id) => Some(LaboratoryId::parse(lab_id).map_err(|e| anyhow!("{e}"))?),
            None => None,
        };
        Ok(Some(Actor {
            user_id,
            user_type,
            laboratory_id,
        }))
    } else {
        Ok(None)
    }
}
