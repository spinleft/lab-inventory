use crate::domain::{
    AssetCategoryId, AssetId, AssetParameterId, AttachmentId, BorrowRequestId, FileUploadId,
    InventoryItemId, LaboratoryId, LocationId, UnitId, UserId, UserRole, UserType,
};
use actix_web::dev::Payload;
use actix_web::error::InternalError;
use actix_web::{FromRequest, HttpMessage, HttpRequest, HttpResponse};
use anyhow::Context;
use anyhow::anyhow;
use sqlx::PgPool;
use std::future::{Ready, ready};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Actor {
    pub user_id: UserId,
    pub user_type: UserType,
    pub laboratory_id: Option<LaboratoryId>,
}

impl FromRequest for Actor {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        match req.extensions().get::<Actor>() {
            Some(actor) => ready(Ok(actor.clone())),
            None => ready(Err(InternalError::from_response(
                anyhow!("Actor was not found in request extensions"),
                HttpResponse::Unauthorized()
                    .json(serde_json::json!({ "error": "Authentication required" })),
            )
            .into())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaboratoryContext {
    actor: Actor,
    laboratory_id: LaboratoryId,
}

impl LaboratoryContext {
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn authorization_actor(&self) -> Actor {
        if self.actor.is_system_admin() {
            Actor {
                user_id: self.actor.user_id,
                user_type: UserType::LabAdmin,
                laboratory_id: Some(self.laboratory_id),
            }
        } else {
            self.actor.clone()
        }
    }

    pub fn laboratory_id(&self) -> LaboratoryId {
        self.laboratory_id
    }
}

impl Deref for LaboratoryContext {
    type Target = LaboratoryId;

    fn deref(&self) -> &Self::Target {
        &self.laboratory_id
    }
}

impl std::fmt::Display for LaboratoryContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.laboratory_id.fmt(f)
    }
}

impl FromRequest for LaboratoryContext {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let Some(actor) = req.extensions().get::<Actor>().cloned() else {
            return ready(Err(InternalError::from_response(
                anyhow!("Actor was not found in request extensions"),
                HttpResponse::Unauthorized()
                    .json(serde_json::json!({ "error": "Authentication required" })),
            )
            .into()));
        };

        if let Some(raw_laboratory_id) = req.match_info().get("laboratory_id") {
            if !actor.is_system_admin() {
                return ready(Err(actix_web::error::ErrorForbidden(
                    "System administrator permissions are required",
                )));
            }
            let laboratory_id = match Uuid::parse_str(raw_laboratory_id) {
                Ok(laboratory_id) => laboratory_id.into(),
                Err(error) => return ready(Err(actix_web::error::ErrorBadRequest(error))),
            };
            return ready(Ok(Self {
                actor,
                laboratory_id,
            }));
        }

        match actor.laboratory_id {
            Some(laboratory_id) if !actor.is_system_admin() => ready(Ok(Self {
                actor,
                laboratory_id,
            })),
            _ => ready(Err(actix_web::error::ErrorForbidden(
                "A laboratory-scoped user is required",
            ))),
        }
    }
}

/// Lets a domain identifier be extracted straight out of a route parameter, so a
/// handler asks for the type it is going to work with rather than a `Uuid` it
/// has to convert. The identifiers themselves stay in `domain`, which knows
/// nothing about Actix; only this impl does.
macro_rules! route_uuid {
    ($name:ident, $parameter:literal) => {
        impl FromRequest for $name {
            type Error = actix_web::Error;
            type Future = Ready<Result<Self, Self::Error>>;

            fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
                let Some(value) = req.match_info().get($parameter) else {
                    return ready(Err(actix_web::error::ErrorBadRequest(concat!(
                        "Missing route parameter: ",
                        $parameter
                    ))));
                };
                ready(
                    Uuid::parse_str(value)
                        .map(Into::into)
                        .map_err(actix_web::error::ErrorBadRequest),
                )
            }
        }
    };
}

route_uuid!(AssetId, "asset_id");
route_uuid!(AssetCategoryId, "category_id");
route_uuid!(AssetParameterId, "parameter_id");
route_uuid!(AttachmentId, "attachment_id");
route_uuid!(BorrowRequestId, "borrow_request_id");
route_uuid!(FileUploadId, "upload_id");
route_uuid!(InventoryItemId, "inventory_item_id");
route_uuid!(LocationId, "location_id");
route_uuid!(UnitId, "unit_id");

pub enum ResourceType {
    Laboratory,
    User,
    GuestRegistrationCode,
    Unit,
    Location,
    Asset,
    InventoryItem,
    AssetCategory,
    AssetParameter,
    AttachmentAssignment,
    FileUpload,
    Federation,
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

    pub fn can_create_guest_registration_code(&self, laboratory_id: LaboratoryId) -> bool {
        matches!(self.user_type, UserType::LabAdmin | UserType::User)
            && self.laboratory_id == Some(laboratory_id)
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
            laboratory_id: LaboratoryId,
            uploaded_by_user_id: UserId,
        }

        let file_upload = sqlx::query_as!(
            FileUploadRow,
            r#"
            SELECT laboratory_id, uploaded_by_user_id
            FROM file_uploads
            WHERE upload_id = $1
            "#,
            upload_id,
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch file upload")?;

        match file_upload {
            Some(file_upload) => Ok((self.is_system_admin()
                || self.laboratory_id == Some(file_upload.laboratory_id))
                && (self.is_system_admin() || self.user_id == file_upload.uploaded_by_user_id)),
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
            laboratory_id: LaboratoryId,
            uploaded_by_user_id: UserId,
        }

        let file_upload = sqlx::query_as!(
            FileUploadRow,
            r#"
            SELECT laboratory_id, uploaded_by_user_id
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
            Some(file_upload) => Ok((self.is_system_admin()
                || self.laboratory_id == Some(file_upload.laboratory_id))
                && (self.is_system_admin() || self.user_id == file_upload.uploaded_by_user_id)),
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
            Ok(self.is_system_admin()
                || (self.laboratory_id == Some(assignment.laboratory_id)
                    && (assignment.is_public || !self.is_guest())))
        } else {
            // Every route that addresses an attachment by id authorizes through
            // `LaboratoryContext::authorization_actor`, which scopes even a system
            // admin to a single laboratory, so no caller here can see every
            // laboratory. A missing attachment is therefore indistinguishable from
            // one the actor may not touch, and both answer 403 so that nobody can
            // probe for attachment ids.
            Ok(false)
        }
    }

    pub fn can_read_laboratory_resource(&self, laboratory_id: &LaboratoryId) -> bool {
        self.is_root() || self.is_super_admin() || self.laboratory_id == Some(*laboratory_id)
    }

    pub fn can_query_laboratory_resource(&self, laboratory_id: &LaboratoryId) -> bool {
        self.can_read_laboratory_resource(laboratory_id)
    }

    pub fn can_browse_laboratories(&self) -> bool {
        self.is_root()
            || self.is_super_admin()
            || ((self.is_lab_admin() || self.is_regular_user()) && self.laboratory_id.is_some())
    }

    /// Federation state is a laboratory's own configuration: only its
    /// administrator may change it.
    pub fn can_manage_federation(&self, laboratory_id: LaboratoryId) -> bool {
        self.is_lab_admin() && self.laboratory_id == Some(laboratory_id)
    }

    /// Reading which laboratories are federated with is open to everyone who can
    /// actually follow those links.
    pub fn can_read_federation(&self, laboratory_id: LaboratoryId) -> bool {
        (self.is_lab_admin() || self.is_regular_user()) && self.laboratory_id == Some(laboratory_id)
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
    actor: &Actor,
    resource_type: ResourceType,
    action: Action<'_>,
) -> Result<bool, anyhow::Error> {
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
            Action::Read(user_id) => Ok(actor.can_view_user(pool, user_id.into()).await?),
            Action::UpdateUser(target_user_role, update_user_role) => Ok(actor
                .can_manage_user(target_user_role)
                && actor.can_manage_user(update_user_role)),
            _ => Ok(false),
        },
        ResourceType::GuestRegistrationCode => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_create_guest_registration_code(laboratory_id.into()))
            }
            _ => Ok(false),
        },
        ResourceType::Unit => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(unit_id) => Ok(actor.can_manage_unit(pool, unit_id.into()).await?),
            Action::Read(unit_id) => Ok(actor.can_view_unit(pool, unit_id.into()).await?),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_browse_laboratory_resource(laboratory_id.into()))
            }
            Action::Update(unit_id) => Ok(actor.can_manage_unit(pool, unit_id.into()).await?),
            _ => Ok(false),
        },
        ResourceType::Location => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(location_id) => {
                Ok(actor.can_manage_location(pool, location_id.into()).await?)
            }
            Action::Read(location_id) => {
                Ok(actor.can_view_location(pool, location_id.into()).await?)
            }
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::Update(location_id) => {
                Ok(actor.can_manage_location(pool, location_id.into()).await?)
            }
            _ => Ok(false),
        },
        ResourceType::Asset => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(asset_id) => Ok(actor.can_manage_asset(pool, asset_id.into()).await?),
            Action::Read(asset_id) => Ok(actor.can_view_asset(pool, asset_id.into()).await?),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::BrowseInternal(laboratory_id) => {
                Ok(actor.can_read_laboratory_resource(&laboratory_id.into()))
            }
            Action::Update(asset_id) => Ok(actor.can_manage_asset(pool, asset_id.into()).await?),
            _ => Ok(false),
        },
        ResourceType::InventoryItem => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(inventory_item_id) => Ok(actor
                .can_manage_inventory_item(pool, inventory_item_id.into())
                .await?),
            Action::Read(inventory_item_id) => Ok(actor
                .can_view_inventory_item(pool, inventory_item_id.into())
                .await?),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::BrowseInternal(laboratory_id) => {
                Ok(actor.can_read_laboratory_resource(&laboratory_id.into()))
            }
            Action::Update(inventory_item_id) => Ok(actor
                .can_manage_inventory_item(pool, inventory_item_id.into())
                .await?),
            _ => Ok(false),
        },
        ResourceType::AssetCategory => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(asset_category_id) => Ok(actor
                .can_manage_asset_category(pool, asset_category_id.into())
                .await?),
            Action::Read(asset_category_id) => Ok(actor
                .can_view_asset_category(pool, asset_category_id.into())
                .await?),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::Update(asset_category_id) => Ok(actor
                .can_manage_asset_category(pool, asset_category_id.into())
                .await?),
            _ => Ok(false),
        },
        ResourceType::AssetParameter => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(asset_parameter_id) => Ok(actor
                .can_manage_asset_parameter(pool, asset_parameter_id.into())
                .await?),
            Action::Read(asset_parameter_id) => Ok(actor
                .can_view_asset_parameter(pool, asset_parameter_id.into())
                .await?),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::Update(asset_parameter_id) => Ok(actor
                .can_manage_asset_parameter(pool, asset_parameter_id.into())
                .await?),
            _ => Ok(false),
        },
        ResourceType::FileUpload => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(upload_id) => Ok(actor.can_manage_file_upload(pool, upload_id).await?),
            Action::Read(_) => Ok(true),
            Action::Browse(laboratory_id) => {
                Ok(actor.can_browse_laboratory_resource(laboratory_id.into()))
            }
            Action::Assign(upload_id) => Ok(actor.can_assign_file_upload(pool, upload_id).await?),
            _ => Ok(false),
        },
        ResourceType::AttachmentAssignment => match action {
            Action::Create(laboratory_id) => {
                Ok(actor.can_write_laboratory_resource(laboratory_id.into()))
            }
            Action::Delete(attachment_id) => Ok(actor
                .can_manage_attachment_assignment(pool, attachment_id)
                .await?),
            Action::Read(attachment_id) => Ok(actor
                .can_view_attachment_assignment(pool, attachment_id)
                .await?),
            // Attachments follow the laboratory scope of their owning resource.
            Action::Browse(laboratory_id) => {
                Ok(actor.can_query_laboratory_resource(&laboratory_id.into()))
            }
            Action::BrowseInternal(laboratory_id) => {
                Ok(actor.can_browse_laboratory_internal_resource(laboratory_id.into()))
            }
            Action::Update(attachment_id) => Ok(actor
                .can_manage_attachment_assignment(pool, attachment_id)
                .await?),
            _ => Ok(false),
        },
        // Federation state is a laboratory's own configuration rather than a set
        // of separately owned rows, so every action is answered from the
        // laboratory it belongs to. The trust and guest link ids stay out of it:
        // the queries that read them are already scoped to that laboratory.
        ResourceType::Federation => match action {
            Action::Create(laboratory_id)
            | Action::Update(laboratory_id)
            | Action::Delete(laboratory_id)
            | Action::BrowseInternal(laboratory_id) => {
                Ok(actor.can_manage_federation(laboratory_id.into()))
            }
            Action::Browse(laboratory_id) => Ok(actor.can_read_federation(laboratory_id.into())),
            _ => Ok(false),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::Actor;
    use crate::domain::{LaboratoryId, UserId, UserType};
    use uuid::Uuid;

    fn actor(user_type: UserType, laboratory_id: Option<LaboratoryId>) -> Actor {
        Actor {
            user_id: UserId(Uuid::new_v4()),
            user_type,
            laboratory_id,
        }
    }

    #[test]
    fn only_lab_admins_and_users_can_create_codes_for_their_own_laboratory() {
        let laboratory_id: LaboratoryId = Uuid::new_v4().into();
        let other_laboratory_id: LaboratoryId = Uuid::new_v4().into();

        for user_type in [UserType::LabAdmin, UserType::User] {
            let actor = actor(user_type, Some(laboratory_id));
            assert!(actor.can_create_guest_registration_code(laboratory_id));
            assert!(!actor.can_create_guest_registration_code(other_laboratory_id));
        }

        for user_type in [UserType::Guest, UserType::Root, UserType::SuperAdmin] {
            let actor = actor(user_type, Some(laboratory_id));
            assert!(!actor.can_create_guest_registration_code(laboratory_id));
        }
    }
}
