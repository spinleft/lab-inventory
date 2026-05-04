use crate::authentication::UserId;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub const OWNER: &str = "owner";
pub const MAINTAINER: &str = "maintainer";
pub const USER: &str = "user";
pub const GUEST: &str = "guest";

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Actor {
    pub user_id: Uuid,
    pub user_type_name: String,
    pub laboratory_id: Option<Uuid>,
}

impl Actor {
    pub fn is_owner(&self) -> bool {
        self.user_type_name == OWNER
    }

    pub fn is_maintainer(&self) -> bool {
        self.user_type_name == MAINTAINER
    }

    pub fn can_manage_user(
        &self,
        target_user_type: &str,
        target_laboratory_id: Option<Uuid>,
    ) -> bool {
        if self.is_owner() {
            return true;
        }

        self.is_maintainer()
            && matches!(target_user_type, USER | GUEST)
            && self.laboratory_id.is_some()
            && self.laboratory_id == target_laboratory_id
    }

    pub fn can_write_laboratory_resource(&self, laboratory_id: Uuid) -> bool {
        self.is_owner()
            || (matches!(self.user_type_name.as_str(), MAINTAINER | USER)
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
            user_types.name AS user_type_name,
            users.laboratory_id
        FROM users
        INNER JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(*user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::Unauthorized)
}

pub async fn user_type_exists(pool: &PgPool, user_type_name: &str) -> Result<bool, ApiError> {
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM user_types WHERE name = $1")
        .bind(user_type_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(exists.is_some())
}

pub fn requires_laboratory(user_type_name: &str) -> bool {
    matches!(user_type_name, MAINTAINER | USER | GUEST)
}
