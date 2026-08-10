use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
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
