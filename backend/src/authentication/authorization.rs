use crate::authentication::UserId;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub const SYSTEM_ADMIN: &str = "system_admin";
pub const LAB_ADMIN: &str = "lab_admin";
pub const USER: &str = "user";
pub const GUEST: &str = "guest";

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Actor {
    pub user_id: Uuid,
    pub group_name: String,
    pub laboratory_id: Option<Uuid>,
}

impl Actor {
    pub fn is_system_admin(&self) -> bool {
        self.group_name == SYSTEM_ADMIN
    }

    pub fn is_lab_admin(&self) -> bool {
        self.group_name == LAB_ADMIN
    }

    pub fn can_manage_user(&self, target_group: &str, target_laboratory_id: Option<Uuid>) -> bool {
        if self.is_system_admin() {
            return true;
        }

        self.is_lab_admin()
            && matches!(target_group, USER | GUEST)
            && self.laboratory_id.is_some()
            && self.laboratory_id == target_laboratory_id
    }

    pub fn can_write_laboratory_resource(&self, laboratory_id: Uuid) -> bool {
        self.is_system_admin()
            || (matches!(self.group_name.as_str(), LAB_ADMIN | USER)
                && self.laboratory_id == Some(laboratory_id))
    }

    pub fn is_same_laboratory(&self, laboratory_id: Uuid) -> bool {
        self.laboratory_id == Some(laboratory_id)
    }
}

pub async fn get_actor(pool: &PgPool, user_id: UserId) -> Result<Actor, ApiError> {
    sqlx::query_as::<_, Actor>(
        r#"
        SELECT
            users.user_id,
            user_groups.name AS group_name,
            users.laboratory_id
        FROM users
        INNER JOIN user_groups USING (group_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(*user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::Unauthorized)
}

pub async fn group_exists(pool: &PgPool, group_name: &str) -> Result<bool, ApiError> {
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM user_groups WHERE name = $1")
        .bind(group_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(exists.is_some())
}

pub fn requires_laboratory(group_name: &str) -> bool {
    matches!(group_name, LAB_ADMIN | USER | GUEST)
}
