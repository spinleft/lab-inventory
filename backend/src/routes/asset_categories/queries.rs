//! Every SQL statement the asset category routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only
//!   [`AssetCategoryDatabaseError`], so any handler can reuse them
use super::model::{
    AssetCategoryParameterAssignmentInput, AssetCategoryParameterAssignmentRow, AssetCategoryRow,
};
use crate::domain::{AssetCategoryId, LaboratoryId};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum AssetCategoryDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for AssetCategoryDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> AssetCategoryDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("uq_asset_categories_sibling_name")) => {
                return AssetCategoryDatabaseError::Conflict(
                    "Asset category name already exists under this parent".into(),
                );
            }
            (Some("23505"), Some("uq_asset_categories_sibling_code")) => {
                return AssetCategoryDatabaseError::Conflict(
                    "Asset category code already exists under this parent".into(),
                );
            }
            (Some("23505"), Some("uq_asset_categories_path")) => {
                return AssetCategoryDatabaseError::Conflict(
                    "Asset category path already exists".into(),
                );
            }
            (Some("23505"), _) => {
                return AssetCategoryDatabaseError::Conflict(
                    "Asset category already exists".into(),
                );
            }
            (Some("23503"), _) => {
                return AssetCategoryDatabaseError::Validation("Invalid laboratory".into());
            }
            _ => {}
        }
    }

    AssetCategoryDatabaseError::Unexpected(error.into())
}

/// A category other rows still point at cannot be removed. That is a conflict
/// rather than bad input, so the delete path maps the foreign key violation
/// differently from [`map_database_error`].
fn map_delete_error(error: sqlx::Error) -> AssetCategoryDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return AssetCategoryDatabaseError::Conflict(
            "Asset category is referenced by other records".into(),
        );
    }

    map_database_error(error)
}

// ---------------------------------------------------------------------------
// asset categories
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_category(
    pool: &PgPool,
    category_id: AssetCategoryId,
) -> Result<Option<AssetCategoryRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        SELECT
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
        FROM asset_categories
        WHERE category_id = $1
        "#,
        Uuid::from(category_id),
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch asset category")
}

/// Same projection as [`fetch_asset_category`], but takes the row lock the write
/// paths need. `query_as!` requires a literal, so the column list cannot be
/// shared.
pub(super) async fn fetch_asset_category_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: AssetCategoryId,
) -> Result<Option<AssetCategoryRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        SELECT
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
        FROM asset_categories
        WHERE category_id = $1
        FOR UPDATE
        "#,
        Uuid::from(category_id),
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset category for update")
}

pub(super) async fn fetch_asset_categories(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    root_path: Option<&str>,
) -> Result<Vec<AssetCategoryRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        SELECT
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
        FROM asset_categories
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset categories")
}

pub(super) async fn fetch_asset_category_tree_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<AssetCategoryRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        SELECT
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
        FROM asset_categories
        WHERE laboratory_id = $1
          AND path <@ $2::text::ltree
        ORDER BY path
        FOR UPDATE
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset category tree for update")
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Saving new asset category in the database",
    skip(transaction, name, code, path, description),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_asset_category(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_category_id: Option<AssetCategoryId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<AssetCategoryRow, AssetCategoryDatabaseError> {
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

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating asset category in the database",
    skip(transaction, name, code, path, description),
    fields(category_id=%category_id)
)]
pub(super) async fn update_asset_category_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
    parent_category_id: Option<Uuid>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<AssetCategoryRow, AssetCategoryDatabaseError> {
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

/// Rewrites every descendant path so it keeps hanging off `new_path`, and
/// recomputes the depth that follows from it.
#[tracing::instrument(
    name = "Updating asset category descendant paths in the database",
    skip(transaction, old_path, new_path),
    fields(laboratory_id=%laboratory_id, category_id=%category_id)
)]
pub(super) async fn update_descendant_paths(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    old_path: &str,
    new_path: &str,
) -> Result<(), AssetCategoryDatabaseError> {
    sqlx::query!(
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
        laboratory_id,
        new_path,
        old_path,
        category_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

#[tracing::instrument(
    name = "Deleting asset category tree from the database",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn delete_asset_category_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), AssetCategoryDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM asset_categories
        WHERE laboratory_id = $1
          AND path <@ $2::text::ltree
        "#,
        *laboratory_id,
        root_path,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_delete_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// parameter assignments
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_category_parameter_assignments(
    pool: &PgPool,
    category_id: Uuid,
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
        r#"
        SELECT
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        FROM asset_parameter_assignments
        WHERE category_id = $1
        ORDER BY sort_order, parameter_type_id
        "#,
    )
    .bind(category_id)
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset category parameter assignments")
}

pub(super) async fn fetch_asset_category_parameter_assignments_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
        r#"
        SELECT
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        FROM asset_parameter_assignments
        WHERE category_id = $1
        ORDER BY sort_order, parameter_type_id
        FOR UPDATE
        "#,
    )
    .bind(category_id)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset category parameter assignments for update")
}

pub(super) async fn fetch_asset_category_parameter_assignments_for_categories(
    pool: &PgPool,
    category_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AssetCategoryParameterAssignmentRow>>, anyhow::Error> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
        r#"
        SELECT
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        FROM asset_parameter_assignments
        WHERE category_id = ANY($1)
        ORDER BY category_id, sort_order, parameter_type_id
        "#,
    )
    .bind(category_ids)
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset category parameter assignments")?;

    let mut assignments_by_category_id: HashMap<Uuid, Vec<_>> = HashMap::new();
    for row in rows {
        assignments_by_category_id
            .entry(row.category_id)
            .or_default()
            .push(row);
    }

    Ok(assignments_by_category_id)
}

pub(super) async fn fetch_asset_category_parameter_assignments_for_categories_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    category_ids: &[Uuid],
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, anyhow::Error> {
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
        r#"
        SELECT
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        FROM asset_parameter_assignments
        WHERE category_id = ANY($1)
        ORDER BY category_id, sort_order, parameter_type_id
        FOR UPDATE
        "#,
    )
    .bind(category_ids)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset category parameter assignments for update")
}

/// Of the ids handed in, the ones that really are parameters of this laboratory.
/// A shorter result than the input means at least one id was foreign to it.
pub(super) async fn fetch_asset_parameter_ids_for_laboratory(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    parameter_type_ids: &[Uuid],
) -> Result<Vec<Uuid>, anyhow::Error> {
    if parameter_type_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT parameter_type_id
        FROM asset_parameter_types
        WHERE laboratory_id = $1
          AND parameter_type_id = ANY($2)
        "#,
    )
    .bind(laboratory_id)
    .bind(parameter_type_ids)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameters for asset category assignment")
}

pub(super) async fn insert_asset_category_parameter_assignment(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    assignment: &AssetCategoryParameterAssignmentInput,
) -> Result<AssetCategoryParameterAssignmentRow, anyhow::Error> {
    sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
        r#"
        INSERT INTO asset_parameter_assignments (
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(assignment.parameter_type_id)
    .bind(category_id)
    .bind(assignment.applies_to_descendants)
    .bind(assignment.is_required)
    .bind(assignment.sort_order)
    .fetch_one(transaction.as_mut())
    .await
    .context("Failed to insert asset category parameter assignment")
}

pub(super) async fn delete_asset_category_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        "DELETE FROM asset_parameter_assignments WHERE category_id = $1",
        category_id,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to delete existing asset category parameter assignments")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// assets filed under a category
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_ids_in_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, AssetCategoryDatabaseError> {
    sqlx::query_scalar!(
        r#"
        SELECT asset_id
        FROM assets
        WHERE laboratory_id = $1
          AND category_id IN (
              SELECT category_id
              FROM asset_categories
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        ORDER BY asset_id
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn clear_asset_categories_in_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), AssetCategoryDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE assets
        SET category_id = NULL,
            updated_at = now()
        WHERE laboratory_id = $1
          AND category_id IN (
              SELECT category_id
              FROM asset_categories
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        "#,
        *laboratory_id,
        root_path,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}
