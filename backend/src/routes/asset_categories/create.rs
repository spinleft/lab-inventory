use super::model::{
    AssetCategoryParameterAssignmentInput, AssetCategoryResponse,
    create_asset_category_rollback_details,
};
use super::queries::{AssetCategoryDatabaseError, insert_asset_category};
use super::service::{
    build_path_and_depth, insert_parameter_assignments, resolve_new_parent,
    validate_parameter_assignments,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetCategoryCode, AssetCategoryName, NewAssetCategory};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    parent_category_id: Option<Uuid>,
    name: String,
    code: String,
    description: Option<String>,
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

impl TryFrom<JsonData> for NewAssetCategory {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_category_id = value.parent_category_id.map(Uuid::into);
        let name = AssetCategoryName::parse(value.name)?;
        let code = AssetCategoryCode::parse(value.code)?;

        Ok(Self {
            parent_category_id,
            name,
            code,
            description: value.description,
        })
    }
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

#[derive(thiserror::Error)]
pub enum CreateAssetCategoryError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateAssetCategoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateAssetCategoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateAssetCategoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateAssetCategoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateAssetCategoryError::ConflictError(_) => StatusCode::CONFLICT,
            CreateAssetCategoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetCategoryDatabaseError> for CreateAssetCategoryError {
    fn from(error: AssetCategoryDatabaseError) -> Self {
        match error {
            AssetCategoryDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetCategoryDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetCategoryDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create an asset category",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_asset_category(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateAssetCategoryError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::AssetCategory,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(CreateAssetCategoryError::Forbidden(
            "You don't have permission to create asset categories.".into(),
        ));
    }

    let payload = payload.into_inner();
    let parameter_assignments =
        parse_parameter_assignments(payload.parameter_assignments.as_deref().unwrap_or(&[]))
            .map_err(CreateAssetCategoryError::ValidationError)?;
    let new_category =
        NewAssetCategory::try_from(payload).map_err(CreateAssetCategoryError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let parent = resolve_new_parent(
        &mut transaction,
        laboratory_id,
        new_category.parent_category_id,
    )
    .await?;
    validate_parameter_assignments(
        &mut transaction,
        laboratory_id.into(),
        &parameter_assignments,
    )
    .await?;
    let (path, depth) = build_path_and_depth(parent.as_ref(), new_category.code.as_ref());
    let category = insert_asset_category(
        &mut transaction,
        laboratory_id,
        new_category.parent_category_id,
        new_category.name.as_ref(),
        new_category.code.as_ref(),
        &path,
        depth,
        new_category.description.as_deref(),
    )
    .await?;
    let parameter_assignments = insert_parameter_assignments(
        &mut transaction,
        category.laboratory_id,
        category.category_id,
        &parameter_assignments,
    )
    .await?;

    record_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        AuditResource::AssetCategory,
        Some(category.category_id),
        create_asset_category_rollback_details(&category),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new user.")?;

    Ok(
        HttpResponse::Created().json(AssetCategoryResponse::from_parts(
            category,
            parameter_assignments,
        )),
    )
}
