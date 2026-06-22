use super::model::{
    AssetCategoryResponse, AssetCategoryRow,
    create_asset_category_rollback_details, fetch_asset_category_for_update, map_database_conflict,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetCategoryCode, AssetCategoryId, AssetCategoryName, NewAssetCategory};
use crate::domain::{LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Deserialize;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    parent_category_id: Option<Uuid>,
    name: String,
    code: String,
    description: Option<String>,
}

impl TryFrom<JsonData> for NewAssetCategory {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_category_id = value
            .parent_category_id
            .map(AssetCategoryId::parse)
            .transpose()?;
        let name = AssetCategoryName::parse(value.name)?;
        let code = AssetCategoryCode::parse(value.code)?;

        Ok(Self::new(parent_category_id, name, code, value.description))
    }
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

#[tracing::instrument(
    name = "Create an asset category",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_asset_category(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateAssetCategoryError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(CreateAssetCategoryError::ValidationError)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateAssetCategoryError::UnexpectedError)?
        .ok_or(CreateAssetCategoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let new_category = NewAssetCategory::try_from(payload.into_inner())
        .map_err(CreateAssetCategoryError::ValidationError)?;
    validate_create_permission(&actor, &laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let parent = fetch_parent_category(&mut transaction, new_category.parent_category_id).await?;
    validate_parent(&parent, &laboratory_id)?;
    let parent_category_id = new_category.parent_category_id;
    let (path, depth) = build_path_and_depth(parent.as_ref(), new_category.code.as_ref());
    let category = insert_asset_category(
        &mut transaction,
        laboratory_id,
        parent_category_id,
        new_category.name.as_ref(),
        new_category.code.as_ref(),
        &path,
        depth,
        new_category.description.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
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

    Ok(HttpResponse::Created().json(AssetCategoryResponse::from(category)))
}

fn validate_create_permission(actor: &Actor, target_laboratory_id: &LaboratoryId) -> Result<(), CreateAssetCategoryError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(CreateAssetCategoryError::Forbidden(
            "You don't have permission to create asset categories for this laboratory.".into(),
        ))
    }
}

async fn fetch_parent_category(
    transaction: &mut Transaction<'_, Postgres>,
    parent_category_id: Option<AssetCategoryId>,
) -> Result<Option<AssetCategoryRow>, CreateAssetCategoryError> {
    let Some(parent_category_id) = parent_category_id else {
        return Ok(None);
    };

    fetch_asset_category_for_update(transaction, parent_category_id)
        .await?
        .ok_or(CreateAssetCategoryError::ValidationError(
            "Parent category not found".into(),
        ))
        .map(Some)
}

fn validate_parent(parent: &Option<AssetCategoryRow>, laboratory_id: &LaboratoryId) -> Result<(), CreateAssetCategoryError> {
    if let Some(parent) = parent {
        if &LaboratoryId::parse(parent.laboratory_id)
            .map_err(CreateAssetCategoryError::ValidationError)?
            != laboratory_id
        {
            return Err(CreateAssetCategoryError::ValidationError(
                "Parent category does not belong to this laboratory".into(),
            ));
        }
    }
    Ok(())
}

fn build_path_and_depth(parent: Option<&AssetCategoryRow>, code: &str) -> (String, i32) {
    match parent {
        Some(parent) => (format!("{}.{}", parent.path, code), parent.depth + 1),
        None => (code.to_string(), 0),
    }
}

#[tracing::instrument(
    name = "Saving new asset category in the database",
    skip(transaction, name, code, path, description),
    fields(laboratory_id=%laboratory_id)
)]
async fn insert_asset_category(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_category_id: Option<AssetCategoryId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<AssetCategoryRow, CreateAssetCategoryError> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        INSERT INTO asset_categories (
            category_id,
            laboratory_id,
            parent_category_id,
            name,
            code,
            path,
            depth,
            description
        )
        VALUES ($1, $2, $3, $4, $5, $6::text::ltree, $7, $8)
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
        Uuid::new_v4(),
        *laboratory_id,
        parent_category_id.map(Uuid::from),
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

fn map_database_error(error: sqlx::Error) -> CreateAssetCategoryError {
    if let Some(message) = map_database_conflict(
        &error,
        "Asset category name already exists under this parent",
        "Asset category code already exists under this parent",
        "Asset category path already exists",
        "Asset category already exists",
    ) {
        return CreateAssetCategoryError::ConflictError(message);
    }

    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return CreateAssetCategoryError::ValidationError("Invalid laboratory".into());
        }
    }

    CreateAssetCategoryError::UnexpectedError(error.into())
}
