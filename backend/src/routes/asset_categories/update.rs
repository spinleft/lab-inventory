use super::model::{
    AssetCategoryParameterAssignmentInput, AssetCategoryResponse, AssetCategoryRow,
    fetch_asset_category_for_update, fetch_asset_category_parameter_assignments_for_update,
    fetch_asset_parameter_ids_for_laboratory, map_database_conflict,
    replace_asset_category_parameter_assignments, update_asset_category_rollback_details,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetCategoryCode, AssetCategoryId, AssetCategoryName, LaboratoryId, NullableUpdate,
    UpdateAssetCategory, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, Postgres, Transaction};
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
        let parent_category_id =
            parse_nullable_update(value.parent_category_id, AssetCategoryId::parse)?;
        let name = value.name.map(AssetCategoryName::parse).transpose()?;
        let code = value.code.map(AssetCategoryCode::parse).transpose()?;
        let description = match value.description {
            Some(Some(description)) => NullableUpdate::Set(description),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };

        Ok(Self::new(parent_category_id, name, code, description))
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn parse_nullable_update<T, V>(
    value: Option<Option<V>>,
    parse: impl FnOnce(V) -> Result<T, String>,
) -> Result<NullableUpdate<T>, String> {
    match value {
        Some(Some(value)) => parse(value).map(NullableUpdate::Set),
        Some(None) => Ok(NullableUpdate::Clear),
        None => Ok(NullableUpdate::Unchanged),
    }
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

#[tracing::instrument(
    name = "Update an asset category",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, category_id=%category_id)
)]
pub async fn update_asset_category(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<AssetCategoryId>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetCategoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateAssetCategoryError::UnexpectedError)?
        .ok_or(UpdateAssetCategoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
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
    let existing = fetch_asset_category_for_update(&mut transaction, *category_id)
        .await?
        .ok_or(UpdateAssetCategoryError::NotFound(
            "Asset category not found".into(),
        ))?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| UpdateAssetCategoryError::UnexpectedError(anyhow::anyhow!("{e}")))?;
    validate_update_permission(&actor, &laboratory_id)?;
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
    let current_parent_category_id = existing
        .parent_category_id
        .map(AssetCategoryId::parse)
        .transpose()
        .map_err(UpdateAssetCategoryError::ValidationError)?;
    let parent_category_id = update_category
        .parent_category_id
        .resolve(current_parent_category_id);
    let description = update_category
        .description
        .resolve(existing.description.clone());

    let parent: Option<AssetCategoryRow> =
        fetch_new_parent(&mut transaction, &existing, parent_category_id).await?;
    let (path, depth) = build_path_and_depth(parent.as_ref(), &code);
    let updated = update_asset_category_in_database(
        &mut transaction,
        existing.category_id,
        parent_category_id.map(Uuid::from),
        &name,
        &code,
        &path,
        depth,
        description.as_deref(),
    )
    .await?;

    if updated.path != existing.path || updated.depth != existing.depth {
        update_descendant_paths(
            &mut transaction,
            existing.laboratory_id,
            existing.category_id,
            &existing.path,
            &updated.path,
        )
        .await?;
    }
    let parameter_assignments = match parameter_assignments {
        Some(parameter_assignments) => {
            replace_asset_category_parameter_assignments(
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
        &actor,
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

fn validate_update_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), UpdateAssetCategoryError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(UpdateAssetCategoryError::Forbidden(
            "You don't have permission to update asset categories for this laboratory.".into(),
        ))
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

async fn validate_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<(), UpdateAssetCategoryError> {
    let parameter_type_ids: Vec<_> = assignments
        .iter()
        .map(|assignment| assignment.parameter_type_id)
        .collect();
    let valid_parameter_type_ids =
        fetch_asset_parameter_ids_for_laboratory(transaction, laboratory_id, &parameter_type_ids)
            .await?;

    if valid_parameter_type_ids.len() != parameter_type_ids.len() {
        return Err(UpdateAssetCategoryError::ValidationError(
            "Asset parameter does not belong to this laboratory".into(),
        ));
    }

    Ok(())
}

async fn fetch_new_parent(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &AssetCategoryRow,
    parent_category_id: Option<AssetCategoryId>,
) -> Result<Option<AssetCategoryRow>, UpdateAssetCategoryError> {
    let Some(parent_category_id) = parent_category_id else {
        return Ok(None);
    };
    if Uuid::from(parent_category_id) == existing.category_id {
        return Err(UpdateAssetCategoryError::ValidationError(
            "Asset category cannot be moved under itself".into(),
        ));
    }

    let parent = fetch_asset_category_for_update(transaction, parent_category_id)
        .await?
        .ok_or(UpdateAssetCategoryError::ValidationError(
            "Parent category not found".into(),
        ))?;
    if parent.laboratory_id != existing.laboratory_id {
        return Err(UpdateAssetCategoryError::ValidationError(
            "Parent category does not belong to this laboratory".into(),
        ));
    }
    if path_is_self_or_descendant(&parent.path, &existing.path) {
        return Err(UpdateAssetCategoryError::ValidationError(
            "Asset category cannot be moved under one of its descendants".into(),
        ));
    }

    Ok(Some(parent))
}

fn path_is_self_or_descendant(candidate_path: &str, root_path: &str) -> bool {
    candidate_path == root_path
        || candidate_path
            .strip_prefix(root_path)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn build_path_and_depth(parent: Option<&AssetCategoryRow>, code: &str) -> (String, i32) {
    match parent {
        Some(parent) => (format!("{}.{}", parent.path, code), parent.depth + 1),
        None => (code.to_string(), 0),
    }
}

#[tracing::instrument(
    name = "Updating asset category in the database",
    skip(transaction, name, code, path, description),
    fields(category_id=%category_id)
)]
async fn update_asset_category_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
    parent_category_id: Option<Uuid>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<AssetCategoryRow, UpdateAssetCategoryError> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        UPDATE asset_categories
        SET
            parent_category_id = $2,
            name = $3,
            code = $4,
            path = $5::text::ltree,
            depth = $6,
            description = $7,
            updated_at = now()
        WHERE category_id = $1
        RETURNING
            category_id,
            laboratory_id,
            parent_category_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        "#,
        category_id,
        parent_category_id,
        name,
        code,
        path,
        depth,
        description,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(
    name = "Updating asset category descendant paths in the database",
    skip(transaction, old_path, new_path),
    fields(laboratory_id=%laboratory_id, category_id=%category_id)
)]
async fn update_descendant_paths(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    old_path: &str,
    new_path: &str,
) -> Result<(), UpdateAssetCategoryError> {
    sqlx::query(
        r#"
        UPDATE asset_categories
        SET
            path = ($2::text::ltree || subpath(path, nlevel($3::text::ltree))),
            depth = nlevel($2::text::ltree || subpath(path, nlevel($3::text::ltree))) - 1,
            updated_at = now()
        WHERE laboratory_id = $1
          AND path <@ $3::text::ltree
          AND category_id <> $4
        "#,
    )
    .bind(laboratory_id)
    .bind(new_path)
    .bind(old_path)
    .bind(category_id)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

fn map_database_error(error: sqlx::Error) -> UpdateAssetCategoryError {
    if let Some(message) = map_database_conflict(
        &error,
        "Asset category name already exists under this parent",
        "Asset category code already exists under this parent",
        "Asset category path already exists",
        "Asset category already exists",
    ) {
        return UpdateAssetCategoryError::ConflictError(message);
    }

    UpdateAssetCategoryError::UnexpectedError(error.into())
}
