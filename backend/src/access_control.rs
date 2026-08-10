use crate::domain::{
    AssetId, AssetParameterId, InventoryItemId, LaboratoryId, LocationId, UnitId, UserId, UserRole,
    UserType,
};
use anyhow::Context;
use anyhow::Ok;
use anyhow::anyhow;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Actor {
    pub user_id: UserId,
    pub user_type: UserType,
    pub laboratory_id: Option<LaboratoryId>,
}

pub enum ResourceType {
    Laboratory,
    User,
    Unit,
    Location,
    Asset,
    InventoryItem,
    AssetCategory,
    AssetParameter,
    AttachmentAssignment,
    FileUpload,
}

pub enum Action<'a> {
    Assign(Uuid),
    Create(Uuid),
    CreateUser(&'a UserRole),
    Delete(Uuid),
    DeleteUser(&'a UserRole),
    Read(Uuid),
    Browse(Uuid),
    BrowseInternal(Uuid),
    Update(Uuid),
    UpdateUser(&'a UserRole, &'a UserRole),
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

    pub fn is_system_admin(&self) -> bool {
        matches!(self.user_type, UserType::Root | UserType::SuperAdmin)
    }

    pub fn is_lab_admin(&self) -> bool {
        self.user_type == UserType::LabAdmin
    }

    pub fn is_regular_user(&self) -> bool {
        self.user_type == UserType::User
    }

    pub fn is_super_admin(&self) -> bool {
        self.user_type == UserType::SuperAdmin
    }

    pub fn is_root(&self) -> bool {
        self.user_type == UserType::Root
    }

    pub fn can_manage_user(&self, target_user: &UserRole) -> bool {
        if !self.is_admin() {
            return false;
        }
        // Laboratory-scoped admins cannot manage root or super_admins
        if self.is_lab_admin() {
            if matches!(target_user.user_type, UserType::Root | UserType::SuperAdmin) {
                return false;
            } else {
                if let Some(lab_id) = target_user.laboratory_id {
                    return self.laboratory_id == Some(lab_id);
                } else {
                    // If no lab specified for target, lab admins cannot manage them
                    return false;
                }
            }
        }
        // Super admin cannot manage root users
        if self.is_super_admin() {
            if target_user.user_type == UserType::Root {
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

    pub async fn can_view_user(
        &self,
        pool: &PgPool,
        target_user_id: UserId,
    ) -> Result<bool, anyhow::Error> {
        if let Some(target_user) = get_actor(pool, target_user_id).await? {
            let target_user_type = target_user.user_type;
            let target_laboratory_id = target_user.laboratory_id;
            if self.user_id == target_user_id {
                return Ok(true);
            }
            if self.is_guest() {
                // Guest users can only view their own information
                return Ok(false);
            }

            if self.is_root() {
                // Root can view all users
                return Ok(true);
            }
            if self.is_admin() {
                // Lab admins can view users in their lab and all super_admins and guests
                if self.is_lab_admin() {
                    if matches!(target_user_type, UserType::SuperAdmin | UserType::Root) {
                        return Ok(false);
                    } else {
                        if let Some(lab_id) = target_laboratory_id {
                            return Ok(self.laboratory_id == Some(lab_id));
                        } else {
                            return Ok(false);
                        }
                    }
                }
                // Super admin can view all users except root
                if self.is_super_admin() {
                    if target_user_type == UserType::Root {
                        return Ok(false);
                    } else {
                        return Ok(true);
                    }
                }
            } else {
                // Non-admin users can only view users in their lab
                if let Some(lab_id) = self.laboratory_id {
                    return Ok(target_laboratory_id == Some(lab_id));
                } else {
                    return Ok(false);
                }
            }
        }
        Ok(false)
    }

    pub fn can_write_laboratory_resource(&self, laboratory_id: LaboratoryId) -> bool {
        // Guest users cannot write
        if self.is_guest() {
            return false;
        }
        self.is_system_admin() || self.laboratory_id == Some(laboratory_id)
    }

    pub fn can_browse_laboratory_resource(&self, laboratory_id: LaboratoryId) -> bool {
        self.is_system_admin() || self.laboratory_id == Some(laboratory_id)
    }

    pub fn can_browse_laboratory_internal_resource(&self, laboratory_id: LaboratoryId) -> bool {
        self.is_system_admin() || (self.laboratory_id == Some(laboratory_id) && !self.is_guest())
    }

    pub async fn can_manage_unit(
        &self,
        pool: &PgPool,
        unit_id: UnitId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct UnitLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            UnitLaboratory,
            r#"
            SELECT laboratory_id
            FROM units
            WHERE unit_id = $1
            "#,
            Uuid::from(unit_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch unit")?
        .map(|unit| {
            !self.is_guest()
                && (self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        })
        .unwrap_or(false))
    }

    pub async fn can_view_unit(
        &self,
        pool: &PgPool,
        unit_id: UnitId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct UnitLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            UnitLaboratory,
            r#"
            SELECT laboratory_id
            FROM units
            WHERE unit_id = $1
            "#,
            Uuid::from(unit_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch unit")?
        .map(|unit| self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_manage_location(
        &self,
        pool: &PgPool,
        location_id: LocationId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct LocationLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            LocationLaboratory,
            r#"
            SELECT laboratory_id
            FROM locations
            WHERE location_id = $1
            "#,
            Uuid::from(location_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch location")?
        .map(|unit| {
            !self.is_guest()
                && (self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        })
        .unwrap_or(false))
    }

    pub async fn can_view_location(
        &self,
        pool: &PgPool,
        location_id: LocationId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct LocationLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            LocationLaboratory,
            r#"
            SELECT laboratory_id
            FROM locations
            WHERE location_id = $1
            "#,
            Uuid::from(location_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch location")?
        .map(|location| self.can_query_laboratory_resource(&location.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_manage_asset_category(
        &self,
        pool: &PgPool,
        category_id: AssetParameterId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetCategoryLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetCategoryLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_categories
            WHERE category_id = $1
            "#,
            Uuid::from(category_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset category")?
        .map(|unit| {
            !self.is_guest()
                && (self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        })
        .unwrap_or(false))
    }

    pub async fn can_view_asset_category(
        &self,
        pool: &PgPool,
        category_id: AssetParameterId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetCategoryLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetCategoryLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_categories
            WHERE category_id = $1
            "#,
            Uuid::from(category_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset category")?
        .map(|category| self.can_query_laboratory_resource(&category.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_manage_asset_parameter(
        &self,
        pool: &PgPool,
        parameter_id: AssetParameterId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetParameterLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetParameterLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_parameter_types
            WHERE parameter_type_id = $1
            "#,
            Uuid::from(parameter_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset parameter")?
        .map(|unit| {
            !self.is_guest()
                && (self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        })
        .unwrap_or(false))
    }

    pub async fn can_view_asset_parameter(
        &self,
        pool: &PgPool,
        parameter_id: AssetParameterId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetParameterLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetParameterLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_parameter_types
            WHERE parameter_type_id = $1
            "#,
            Uuid::from(parameter_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset parameter")?
        .map(|parameter| self.can_query_laboratory_resource(&parameter.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_manage_asset(
        &self,
        pool: &PgPool,
        asset_id: AssetId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetLaboratory,
            r#"
            SELECT laboratory_id
            FROM assets
            WHERE asset_id = $1
            "#,
            Uuid::from(asset_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset")?
        .map(|unit| {
            !self.is_guest()
                && (self.is_system_admin() || self.laboratory_id == Some(unit.laboratory_id))
        })
        .unwrap_or(false))
    }

    pub async fn can_view_asset(
        &self,
        pool: &PgPool,
        asset_id: AssetId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AssetLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            AssetLaboratory,
            r#"
            SELECT laboratory_id
            FROM assets
            WHERE asset_id = $1
            "#,
            Uuid::from(asset_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch asset")?
        .map(|asset| self.can_query_laboratory_resource(&asset.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_manage_inventory_item(
        &self,
        pool: &PgPool,
        inventory_item_id: InventoryItemId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct InventoryItemLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            InventoryItemLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_inventory_items
            WHERE inventory_item_id = $1
            "#,
            Uuid::from(inventory_item_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch inventory item")?
        .map(|item| self.can_write_laboratory_resource(item.laboratory_id))
        .unwrap_or(false))
    }

    pub async fn can_view_inventory_item(
        &self,
        pool: &PgPool,
        inventory_item_id: InventoryItemId,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct InventoryItemLaboratory {
            laboratory_id: LaboratoryId,
        }

        Ok(sqlx::query_as!(
            InventoryItemLaboratory,
            r#"
            SELECT laboratory_id
            FROM asset_inventory_items
            WHERE inventory_item_id = $1
            "#,
            Uuid::from(inventory_item_id),
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch inventory item")?
        .map(|item| self.can_query_laboratory_resource(&item.laboratory_id))
        .unwrap_or(false))
    }

    /// Answers only the ownership question: may this actor delete the upload?
    ///
    /// Like [`Actor::can_assign_file_upload`], a missing upload is reported as
    /// permitted so the deleting transaction — which holds the row lock — answers
    /// with a specific 404 instead of it being flattened into a 403.
    pub async fn can_manage_file_upload(
        &self,
        pool: &PgPool,
        upload_id: Uuid,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct FileUploadRow {
            uploaded_by_user_id: UserId,
        }

        let file_upload = sqlx::query_as!(
            FileUploadRow,
            r#"
            SELECT uploaded_by_user_id
            FROM file_uploads
            WHERE upload_id = $1
            "#,
            upload_id,
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch file upload")?;

        match file_upload {
            Some(file_upload) => {
                Ok(self.is_system_admin() || self.user_id == file_upload.uploaded_by_user_id)
            }
            None => Ok(true),
        }
    }

    /// Answers only the ownership question: may this actor turn the upload into an
    /// attachment?
    ///
    /// An upload that is missing, expired or already consumed is reported as
    /// permitted, so that the assigning transaction — which holds the row lock —
    /// answers with the specific 404 / 400 / 409 instead of it being flattened into
    /// a 403. That check is unlocked and racy anyway, so it cannot be authoritative.
    pub async fn can_assign_file_upload(
        &self,
        pool: &PgPool,
        upload_id: Uuid,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct FileUploadRow {
            uploaded_by_user_id: UserId,
        }

        let file_upload = sqlx::query_as!(
            FileUploadRow,
            r#"
            SELECT uploaded_by_user_id
            FROM file_uploads
            WHERE upload_id = $1
              AND consumed_at IS NULL
              AND expires_at > now()
            "#,
            upload_id,
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch file upload")?;

        match file_upload {
            Some(file_upload) => {
                Ok(self.is_system_admin() || self.user_id == file_upload.uploaded_by_user_id)
            }
            None => Ok(true),
        }
    }

    pub async fn can_manage_attachment_assignment(
        &self,
        pool: &PgPool,
        attachment_id: Uuid,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AttachmentAssignmentRow {
            laboratory_id: LaboratoryId,
        }
        if self.is_guest() {
            return Ok(false);
        }

        let assignment = sqlx::query_as!(
            AttachmentAssignmentRow,
            r#"
            SELECT laboratory_id
            FROM asset_attachment_assignments
            WHERE attachment_id = $1
            "#,
            attachment_id,
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch attachment assignment")?;

        if let Some(assignment) = assignment {
            Ok(self.is_system_admin() || self.laboratory_id == Some(assignment.laboratory_id))
        } else {
            Ok(false)
        }
    }

    pub async fn can_view_attachment_assignment(
        &self,
        pool: &PgPool,
        attachment_id: Uuid,
    ) -> Result<bool, anyhow::Error> {
        #[derive(sqlx::FromRow)]
        struct AttachmentAssignmentRow {
            laboratory_id: LaboratoryId,
            is_public: bool,
        }

        let assignment = sqlx::query_as!(
            AttachmentAssignmentRow,
            r#"
            SELECT laboratory_id, is_public
            FROM asset_attachment_assignments
            WHERE attachment_id = $1
            "#,
            attachment_id,
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch attachment assignment")?;

        if let Some(assignment) = assignment {
            if assignment.is_public {
                return Ok(true);
            } else {
                Ok(self.is_system_admin()
                    || (self.laboratory_id == Some(assignment.laboratory_id) && !self.is_guest()))
            }
        } else {
            // A missing attachment is a 404 for actors who can see every
            // laboratory, and stays a 403 for everyone else so that they cannot
            // probe for attachment ids they are not allowed to know about.
            Ok(self.is_system_admin())
        }
    }

    pub fn can_read_laboratory_resource(&self, laboratory_id: &LaboratoryId) -> bool {
        self.is_root() || self.is_super_admin() || self.laboratory_id == Some(*laboratory_id)
    }

    pub fn can_query_laboratory_resource(&self, laboratory_id: &LaboratoryId) -> bool {
        self.can_read_laboratory_resource(laboratory_id)
            || (!self.is_guest() && self.laboratory_id.is_some())
    }

    pub fn can_browse_laboratories(&self) -> bool {
        self.is_root()
            || self.is_super_admin()
            || ((self.is_lab_admin() || self.is_regular_user()) && self.laboratory_id.is_some())
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
        let user_id = UserId(row.user_id.ok_or(anyhow!("Actor user_id is NULL"))?);
        let user_type = UserType::parse(
            &row.user_type_name
                .ok_or(anyhow!("Actor user_type_name is NULL"))?,
        )
        .map_err(|e| anyhow!("{e}"))?;
        let laboratory_id: Option<LaboratoryId> = match row.laboratory_id {
            Some(laboratory_id) => Some(laboratory_id.into()),
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

pub async fn validate_permission(
    pool: &PgPool,
    actor_user_id: &UserId,
    resource_type: ResourceType,
    action: Action<'_>,
) -> Result<bool, anyhow::Error> {
    if let Some(actor) = get_actor(pool, *actor_user_id).await? {
        match resource_type {
            ResourceType::Laboratory => match action {
                Action::Create(_) => Ok(actor.is_system_admin()),
                Action::Delete(_) => Ok(actor.is_system_admin()),
                // A laboratory is administered, not browsed: only admins reach it,
                // and a laboratory-scoped admin is confined to its own.
                Action::Read(laboratory_id) => Ok(actor.is_system_admin()
                    || (actor.is_lab_admin()
                        && actor.laboratory_id.map(Uuid::from) == Some(laboratory_id))),
                Action::Browse(_) => Ok(actor.can_browse_laboratories()),
                Action::Update(laboratory_id) => Ok(actor.is_system_admin()
                    || (actor.is_lab_admin()
                        && actor.laboratory_id.map(Uuid::from) == Some(laboratory_id))),
                _ => Ok(false),
            },
            ResourceType::User => match action {
                Action::CreateUser(user_role) => Ok(actor.can_manage_user(user_role)),
                Action::DeleteUser(user_role) => Ok(actor.can_manage_user(user_role)),
                Action::Read(user_id) => Ok(actor.can_view_user(&pool, user_id.into()).await?),
                Action::UpdateUser(target_user_role, update_user_role) => Ok(actor
                    .can_manage_user(target_user_role)
                    && actor.can_manage_user(update_user_role)),
                _ => Ok(false),
            },
            ResourceType::Unit => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(unit_id) => Ok(actor.can_manage_unit(&pool, unit_id.into()).await?),
                Action::Read(unit_id) => Ok(actor.can_view_unit(&pool, unit_id.into()).await?),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_browse_laboratory_resource(laboratory_id.into()))
                }
                Action::Update(unit_id) => Ok(actor.can_manage_unit(&pool, unit_id.into()).await?),
                _ => Ok(false),
            },
            ResourceType::Location => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(location_id) => {
                    Ok(actor.can_manage_location(&pool, location_id.into()).await?)
                }
                Action::Read(location_id) => {
                    Ok(actor.can_view_location(&pool, location_id.into()).await?)
                }
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::Update(location_id) => {
                    Ok(actor.can_manage_location(&pool, location_id.into()).await?)
                }
                _ => Ok(false),
            },
            ResourceType::Asset => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(asset_id) => {
                    Ok(actor.can_manage_asset(&pool, asset_id.into()).await?)
                }
                Action::Read(asset_id) => Ok(actor.can_view_asset(&pool, asset_id.into()).await?),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::BrowseInternal(laboratory_id) => {
                    Ok(actor.can_read_laboratory_resource(&laboratory_id.into()))
                }
                Action::Update(asset_id) => {
                    Ok(actor.can_manage_asset(&pool, asset_id.into()).await?)
                }
                _ => Ok(false),
            },
            ResourceType::InventoryItem => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(inventory_item_id) => Ok(actor
                    .can_manage_inventory_item(&pool, inventory_item_id.into())
                    .await?),
                Action::Read(inventory_item_id) => Ok(actor
                    .can_view_inventory_item(&pool, inventory_item_id.into())
                    .await?),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::BrowseInternal(laboratory_id) => {
                    Ok(actor.can_read_laboratory_resource(&laboratory_id.into()))
                }
                Action::Update(inventory_item_id) => Ok(actor
                    .can_manage_inventory_item(&pool, inventory_item_id.into())
                    .await?),
                _ => Ok(false),
            },
            ResourceType::AssetCategory => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(asset_category_id) => Ok(actor
                    .can_manage_asset_category(&pool, asset_category_id.into())
                    .await?),
                Action::Read(asset_category_id) => Ok(actor
                    .can_view_asset_category(&pool, asset_category_id.into())
                    .await?),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::Update(asset_category_id) => Ok(actor
                    .can_manage_asset_category(&pool, asset_category_id.into())
                    .await?),
                _ => Ok(false),
            },
            ResourceType::AssetParameter => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(asset_parameter_id) => Ok(actor
                    .can_manage_asset_parameter(&pool, asset_parameter_id.into())
                    .await?),
                Action::Read(asset_parameter_id) => Ok(actor
                    .can_view_asset_parameter(&pool, asset_parameter_id.into())
                    .await?),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::Update(asset_parameter_id) => Ok(actor
                    .can_manage_asset_parameter(&pool, asset_parameter_id.into())
                    .await?),
                _ => Ok(false),
            },
            ResourceType::FileUpload => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(upload_id) => Ok(actor
                    .can_manage_file_upload(&pool, upload_id.into())
                    .await?),
                Action::Read(_) => Ok(true),
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_browse_laboratory_resource(laboratory_id.into()))
                }
                Action::Assign(upload_id) => Ok(actor
                    .can_assign_file_upload(&pool, upload_id.into())
                    .await?),
                _ => Ok(false),
            },
            ResourceType::AttachmentAssignment => match action {
                Action::Create(laboratory_id) => {
                    Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
                }
                Action::Delete(attachment_id) => Ok(actor
                    .can_manage_attachment_assignment(&pool, attachment_id.into())
                    .await?),
                Action::Read(attachment_id) => Ok(actor
                    .can_view_attachment_assignment(&pool, attachment_id)
                    .await?),
                // Attachments follow their asset: readable across laboratories,
                // with the internal ones filtered out by `BrowseInternal`.
                Action::Browse(laboratory_id) => {
                    Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
                }
                Action::BrowseInternal(laboratory_id) => {
                    Ok(actor.can_browse_laboratory_internal_resource(laboratory_id.into()))
                }
                Action::Update(attachment_id) => Ok(actor
                    .can_manage_attachment_assignment(&pool, attachment_id.into())
                    .await?),
                _ => Ok(false),
            },
        }
    } else {
        Ok(false)
    }
}
