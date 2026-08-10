//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
use super::model::{
    AssetCategoryParameterAssignmentInput, AssetCategoryParameterAssignmentRow, AssetCategoryRow,
};
use super::queries::{
    AssetCategoryDatabaseError, clear_asset_categories_in_tree,
    delete_asset_category_parameter_assignments, delete_asset_category_tree,
    fetch_asset_category_for_update, fetch_asset_ids_in_tree,
    fetch_asset_parameter_ids_for_laboratory, insert_asset_category_parameter_assignment,
    update_asset_category_in_database, update_descendant_paths,
};
use crate::domain::{AssetCategoryId, LaboratoryId};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Where a category sits in the tree, derived from its parent and its own code.
/// A category without a parent is a root: its path is just its code.
pub(super) fn build_path_and_depth(parent: Option<&AssetCategoryRow>, code: &str) -> (String, i32) {
    match parent {
        Some(parent) => (format!("{}.{}", parent.path, code), parent.depth + 1),
        None => (code.to_string(), 0),
    }
}

/// Resolves the parent a brand new category is created under, checking that it
/// belongs to the laboratory the category is created in.
pub(super) async fn resolve_new_parent(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_category_id: Option<AssetCategoryId>,
) -> Result<Option<AssetCategoryRow>, AssetCategoryDatabaseError> {
    let Some(parent_category_id) = parent_category_id else {
        return Ok(None);
    };

    let parent = fetch_asset_category_for_update(transaction, parent_category_id)
        .await?
        .ok_or(AssetCategoryDatabaseError::Validation(
            "Parent category not found".into(),
        ))?;
    if parent.laboratory_id != Uuid::from(laboratory_id) {
        return Err(AssetCategoryDatabaseError::Validation(
            "Parent category does not belong to this laboratory".into(),
        ));
    }

    Ok(Some(parent))
}

/// Resolves the parent an existing category is moved under, rejecting the moves
/// that would detach the subtree from the tree by making it its own ancestor.
pub(super) async fn resolve_moved_parent(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &AssetCategoryRow,
    parent_category_id: Option<AssetCategoryId>,
) -> Result<Option<AssetCategoryRow>, AssetCategoryDatabaseError> {
    let Some(parent_category_id) = parent_category_id else {
        return Ok(None);
    };
    if Uuid::from(parent_category_id) == existing.category_id {
        return Err(AssetCategoryDatabaseError::Validation(
            "Asset category cannot be moved under itself".into(),
        ));
    }

    let parent = fetch_asset_category_for_update(transaction, parent_category_id)
        .await?
        .ok_or(AssetCategoryDatabaseError::Validation(
            "Parent category not found".into(),
        ))?;
    if parent.laboratory_id != existing.laboratory_id {
        return Err(AssetCategoryDatabaseError::Validation(
            "Parent category does not belong to this laboratory".into(),
        ));
    }
    if path_is_self_or_descendant(&parent.path, &existing.path) {
        return Err(AssetCategoryDatabaseError::Validation(
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

/// A category can only require parameters its own laboratory defines.
pub(super) async fn validate_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<(), AssetCategoryDatabaseError> {
    let parameter_type_ids: Vec<_> = assignments
        .iter()
        .map(|assignment| assignment.parameter_type_id)
        .collect();
    let valid_parameter_type_ids =
        fetch_asset_parameter_ids_for_laboratory(transaction, laboratory_id, &parameter_type_ids)
            .await?;

    if valid_parameter_type_ids.len() != parameter_type_ids.len() {
        return Err(AssetCategoryDatabaseError::Validation(
            "Asset parameter does not belong to this laboratory".into(),
        ));
    }

    Ok(())
}

/// Writes the assignments one by one and returns them in the order the read
/// paths use, so a caller never has to re-read what it just wrote.
pub(super) async fn insert_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, AssetCategoryDatabaseError> {
    let mut rows = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        let row = insert_asset_category_parameter_assignment(
            transaction,
            laboratory_id,
            category_id,
            assignment,
        )
        .await?;
        rows.push(row);
    }

    rows.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then(left.parameter_type_id.cmp(&right.parameter_type_id))
    });

    Ok(rows)
}

/// The assignment list is replaced wholesale rather than diffed: an update that
/// sends the field states the complete set the category should end up with.
pub(super) async fn replace_parameter_assignments(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    category_id: Uuid,
    assignments: &[AssetCategoryParameterAssignmentInput],
) -> Result<Vec<AssetCategoryParameterAssignmentRow>, AssetCategoryDatabaseError> {
    delete_asset_category_parameter_assignments(transaction, category_id).await?;

    insert_parameter_assignments(transaction, laboratory_id, category_id, assignments).await
}

/// Writes the new shape of a category and, when that moved it in the tree,
/// drags every descendant along so the stored paths stay consistent.
#[allow(clippy::too_many_arguments)]
pub(super) async fn move_asset_category(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &AssetCategoryRow,
    parent_category_id: Option<AssetCategoryId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<AssetCategoryRow, AssetCategoryDatabaseError> {
    let updated = update_asset_category_in_database(
        transaction,
        existing.category_id,
        parent_category_id.map(Uuid::from),
        name,
        code,
        path,
        depth,
        description,
    )
    .await?;

    if updated.path != existing.path || updated.depth != existing.depth {
        update_descendant_paths(
            transaction,
            existing.laboratory_id,
            existing.category_id,
            &existing.path,
            &updated.path,
        )
        .await?;
    }

    Ok(updated)
}

/// Deletes a category together with everything below it.
///
/// Assets are not deleted with the category they are filed under: their
/// `category_id` is cleared first so the delete cannot fail on the foreign key,
/// and the ids are returned so the audit entry can restore them.
pub(super) async fn delete_asset_category_subtree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, AssetCategoryDatabaseError> {
    let cleared_asset_ids = fetch_asset_ids_in_tree(transaction, laboratory_id, root_path).await?;
    clear_asset_categories_in_tree(transaction, laboratory_id, root_path).await?;
    delete_asset_category_tree(transaction, laboratory_id, root_path).await?;

    Ok(cleared_asset_ids)
}
