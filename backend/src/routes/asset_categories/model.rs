use crate::{access_control::Actor, domain::LaboratoryId};
use crate::domain::AssetCategoryId;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
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

impl From<AssetCategoryRow> for AssetCategoryResponse {
    fn from(row: AssetCategoryRow) -> Self {
        Self {
            category_id: row.category_id,
            laboratory_id: row.laboratory_id,
            parent_category_id: row.parent_category_id,
            name: row.name,
            code: row.code,
            path: row.path,
            depth: row.depth,
            description: row.description,
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

pub(super) fn update_asset_category_rollback_details(category: &AssetCategoryRow) -> Value {
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
                "updated_at": category.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_category_rollback_details(
    categories: &[AssetCategoryRow],
    cleared_asset_ids: &[Uuid],
) -> Value {
    json!({
        "rollback": {
            "operation": "restore_tree",
            "resource_type": "asset_category",
            "values": {
                "categories": categories,
                "cleared_asset_ids": cleared_asset_ids,
            },
        },
    })
}

pub(super) fn can_read_laboratory_categories(actor: &Actor, laboratory_id: Uuid) -> bool {
    actor.is_root()
        || actor.is_super_admin()
        || actor.laboratory_id.map(Uuid::from) == Some(laboratory_id)
}

pub(super) fn can_write_laboratory_categories(actor: &Actor, laboratory_id: LaboratoryId) -> bool {
    if actor.is_guest() {
        return false;
    }

    actor.is_root()
        || actor.is_super_admin()
        || actor.laboratory_id == Some(laboratory_id)
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
    laboratory_id: Uuid,
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
        laboratory_id,
        root_path,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset category tree for update")
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
