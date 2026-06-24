use crate::domain::AssetCategoryId;
use crate::domain::LaboratoryId;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct AssetCategoryResponse {
    category_id: Uuid,
    laboratory_id: Uuid,
    parent_category_id: Option<Uuid>,
    name: String,
    code: String,
    path: String,
    depth: i32,
    description: Option<String>,
    parameter_assignments: Vec<AssetCategoryParameterAssignmentResponse>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetCategoryRow {
    pub(super) category_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) parent_category_id: Option<Uuid>,
    pub(super) name: String,
    pub(super) code: String,
    pub(super) path: String,
    pub(super) depth: i32,
    pub(super) description: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetCategoryParameterAssignmentRow {
    pub(super) assignment_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) category_id: Uuid,
    pub(super) applies_to_descendants: bool,
    pub(super) is_required: bool,
    pub(super) sort_order: i32,
    pub(super) created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub(super) struct AssetCategoryParameterAssignmentInput {
    pub(super) parameter_type_id: Uuid,
    pub(super) applies_to_descendants: bool,
    pub(super) is_required: bool,
    pub(super) sort_order: i32,
}

#[derive(Serialize)]
struct AssetCategoryParameterAssignmentResponse {
    assignment_id: Uuid,
    parameter_type_id: Uuid,
    applies_to_descendants: bool,
    is_required: bool,
    sort_order: i32,
}

impl From<AssetCategoryParameterAssignmentRow> for AssetCategoryParameterAssignmentResponse {
    fn from(row: AssetCategoryParameterAssignmentRow) -> Self {
        Self {
            assignment_id: row.assignment_id,
            parameter_type_id: row.parameter_type_id,
            applies_to_descendants: row.applies_to_descendants,
            is_required: row.is_required,
            sort_order: row.sort_order,
        }
    }
}

impl AssetCategoryResponse {
    pub(super) fn from_parts(
        row: AssetCategoryRow,
        parameter_assignments: Vec<AssetCategoryParameterAssignmentRow>,
    ) -> Self {
        Self {
            category_id: row.category_id,
            laboratory_id: row.laboratory_id,
            parent_category_id: row.parent_category_id,
            name: row.name,
            code: row.code,
            path: row.path,
            depth: row.depth,
            description: row.description,
            parameter_assignments: parameter_assignments
                .into_iter()
                .map(AssetCategoryParameterAssignmentResponse::from)
                .collect(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_asset_category_rollback_details(category: &AssetCategoryRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "asset_category",
            "where": {
                "category_id": category.category_id,
            },
        },
    })
}

pub(super) fn update_asset_category_rollback_details(
    category: &AssetCategoryRow,
    parameter_assignments: &[AssetCategoryParameterAssignmentRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "asset_category",
            "where": {
                "category_id": category.category_id,
            },
            "values": {
                "laboratory_id": category.laboratory_id,
                "parent_category_id": category.parent_category_id,
                "name": &category.name,
                "code": &category.code,
                "path": &category.path,
                "depth": category.depth,
                "description": category.description.as_deref(),
                "parameter_assignments": parameter_assignments,
                "updated_at": category.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_category_rollback_details(
    categories: &[AssetCategoryRow],
    cleared_asset_ids: &[Uuid],
    parameter_assignments: &[AssetCategoryParameterAssignmentRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "restore_tree",
            "resource_type": "asset_category",
            "values": {
                "categories": categories,
                "cleared_asset_ids": cleared_asset_ids,
                "parameter_assignments": parameter_assignments,
            },
        },
    })
}

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

pub(super) async fn replace_asset_category_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, anyhow::Error> {
    sqlx::query("DELETE FROM asset_parameter_assignments WHERE category_id = $1")
        .bind(category_id)
        .execute(transaction.as_mut())
        .await
        .context("Failed to delete existing asset category parameter assignments")?;

    insert_asset_category_parameter_assignments(
        transaction,
        laboratory_id,
        category_id,
        assignments,
    )
    .await
}

pub(super) async fn insert_asset_category_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, anyhow::Error> {
    let mut rows = Vec::with_capacity(assignments.len());

    for assignment in assignments {
        let row = sqlx::query_as::<_, AssetCategoryParameterAssignmentRow>(
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
        .context("Failed to insert asset category parameter assignment")?;
        rows.push(row);
    }

    rows.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then(left.parameter_type_id.cmp(&right.parameter_type_id))
    });

    Ok(rows)
}

pub(super) fn map_database_conflict(
    error: &sqlx::Error,
    duplicate_name: &str,
    duplicate_code: &str,
    duplicate_path: &str,
    generic_unique: &str,
) -> Option<String> {
    let sqlx::Error::Database(database_error) = error else {
        return None;
    };

    match (
        database_error.code().as_deref(),
        database_error.constraint(),
    ) {
        (Some("23505"), Some("uq_asset_categories_sibling_name")) => Some(duplicate_name.into()),
        (Some("23505"), Some("uq_asset_categories_sibling_code")) => Some(duplicate_code.into()),
        (Some("23505"), Some("uq_asset_categories_path")) => Some(duplicate_path.into()),
        (Some("23505"), _) => Some(generic_unique.into()),
        _ => None,
    }
}
