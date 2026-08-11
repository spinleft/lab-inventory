use super::model::{
    AssetCategoryParameterAssignmentInput, AssetCategoryResponse,
    update_asset_category_rollback_details,
};
use super::queries::{
    AssetCategoryDatabaseError, fetch_asset_category_for_update,
    fetch_asset_category_parameter_assignments_for_update,
};
use super::service::{
    build_path_and_depth, move_asset_category, replace_parameter_assignments, resolve_moved_parent,
    validate_parameter_assignments,
};
use crate::access_control::AssetCategoryPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetCategoryCode, AssetCategoryId, AssetCategoryName, NullableUpdate, UpdateAssetCategory,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    #[serde(default, deserialize_with = "deserialize_nullable")]
    parent_category_id: Option<Option<Uuid>>,
    name: Option<String>,
    code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    description: Option<Option<String>>,
    parameter_assignments: Option<Vec<ParameterAssignmentJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterAssignmentJsonData {
    parameter_type_id: Uuid,
    applies_to_descendants: Option<bool>,
    is_required: Option<bool>,
    sort_order: Option<i32>,
}

impl TryFrom<JsonData> for UpdateAssetCategory {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_category_id = value.parent_category_id.map(|id| id.map(Uuid::into)).into();
        let name = value.name.map(AssetCategoryName::parse).transpose()?;
        let code = value.code.map(AssetCategoryCode::parse).transpose()?;
        let description = match value.description {
            Some(Some(description)) => NullableUpdate::Set(description),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };
        Ok(Self {
            parent_category_id,
            name,
            code,
            description,
        })
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(thiserror::Error)]
pub enum UpdateAssetCategoryError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UpdateAssetCategoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateAssetCategoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateAssetCategoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateAssetCategoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateAssetCategoryError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateAssetCategoryError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateAssetCategoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetCategoryDatabaseError> for UpdateAssetCategoryError {
    fn from(error: AssetCategoryDatabaseError) -> Self {
        match error {
            AssetCategoryDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetCategoryDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetCategoryDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Update an asset category",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, category_id=%category_id)
)]
pub async fn update_asset_category(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    category_id: AssetCategoryPathId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetCategoryError> {
    let actor = laboratory_context.authorization_actor();
    let category_id: AssetCategoryId = category_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AssetCategory,
        Action::Update(category_id.into()),
    )
    .await?
    {
        return Err(UpdateAssetCategoryError::Forbidden(
            "You are not allowed to update this asset category.".into(),
        ));
    }

    let payload = payload.into_inner();
    let parameter_assignments = payload
        .parameter_assignments
        .as_deref()
        .map(parse_parameter_assignments)
        .transpose()
        .map_err(UpdateAssetCategoryError::ValidationError)?;
    let update_category = UpdateAssetCategory::try_from(payload)
        .map_err(UpdateAssetCategoryError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_category_for_update(&mut transaction, category_id)
        .await?
        .ok_or(UpdateAssetCategoryError::NotFound(
            "Asset category not found".into(),
        ))?;
    let existing_parameter_assignments = fetch_asset_category_parameter_assignments_for_update(
        &mut transaction,
        existing.category_id,
    )
    .await?;
    if let Some(parameter_assignments) = parameter_assignments.as_deref() {
        validate_parameter_assignments(
            &mut transaction,
            existing.laboratory_id,
            parameter_assignments,
        )
        .await?;
    }

    let name = update_category
        .name
        .as_ref()
        .map(|name| name.as_ref())
        .unwrap_or(&existing.name)
        .to_string();
    let code = update_category
        .code
        .as_ref()
        .map(|code| code.as_ref())
        .unwrap_or(&existing.code)
        .to_string();
    let current_parent_category_id = existing.parent_category_id.map(Uuid::into);
    let parent_category_id = update_category
        .parent_category_id
        .resolve(current_parent_category_id);
    let description = update_category
        .description
        .resolve(existing.description.clone());

    let parent = resolve_moved_parent(&mut transaction, &existing, parent_category_id).await?;
    let (path, depth) = build_path_and_depth(parent.as_ref(), &code);
    let updated = move_asset_category(
        &mut transaction,
        &existing,
        parent_category_id,
        &name,
        &code,
        &path,
        depth,
        description.as_deref(),
    )
    .await?;

    let parameter_assignments = match parameter_assignments {
        Some(parameter_assignments) => {
            replace_parameter_assignments(
                &mut transaction,
                updated.laboratory_id,
                updated.category_id,
                &parameter_assignments,
            )
            .await?
        }
        None => existing_parameter_assignments.clone(),
    };

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Update,
        AuditResource::AssetCategory,
        Some(updated.category_id),
        update_asset_category_rollback_details(&existing, &existing_parameter_assignments),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update an asset category.")?;

    Ok(HttpResponse::Ok().json(AssetCategoryResponse::from_parts(
        updated,
        parameter_assignments,
    )))
}

fn parse_parameter_assignments(
    assignments: &[ParameterAssignmentJsonData],
) -> Result<Vec<AssetCategoryParameterAssignmentInput>, String> {
    let mut seen_parameter_ids = HashSet::new();
    let mut parsed = Vec::with_capacity(assignments.len());

    for assignment in assignments {
        if !seen_parameter_ids.insert(assignment.parameter_type_id) {
            return Err("Asset parameter assignments must be unique".into());
        }

        parsed.push(AssetCategoryParameterAssignmentInput {
            parameter_type_id: assignment.parameter_type_id,
            applies_to_descendants: assignment.applies_to_descendants.unwrap_or(true),
            is_required: assignment.is_required.unwrap_or(true),
            sort_order: assignment.sort_order.unwrap_or(0),
        });
    }

    Ok(parsed)
}
