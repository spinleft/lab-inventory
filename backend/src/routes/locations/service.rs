//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
use super::model::LocationRow;
use super::queries::{
    LocationDatabaseError, clear_inventory_item_locations_in_tree, delete_location_tree,
    fetch_inventory_item_ids_in_tree, fetch_location_for_update, update_descendant_paths,
    update_location_in_database,
};
use crate::domain::{LaboratoryId, LocationId};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Where a location sits in the tree, derived from its parent and its own code.
/// A location without a parent is a root: its path is just its code.
pub(super) fn build_path_and_depth(parent: Option<&LocationRow>, code: &str) -> (String, i32) {
    match parent {
        Some(parent) => (format!("{}.{}", parent.path, code), parent.depth + 1),
        None => (code.to_string(), 0),
    }
}

/// Resolves the parent a brand new location is created under, checking that it
/// belongs to the laboratory the location is created in.
pub(super) async fn resolve_new_parent(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_location_id: Option<LocationId>,
) -> Result<Option<LocationRow>, LocationDatabaseError> {
    let Some(parent_location_id) = parent_location_id else {
        return Ok(None);
    };

    let parent = fetch_location_for_update(transaction, parent_location_id)
        .await?
        .ok_or(LocationDatabaseError::Validation(
            "Parent location not found".into(),
        ))?;
    if parent.laboratory_id != Uuid::from(laboratory_id) {
        return Err(LocationDatabaseError::Validation(
            "Parent location does not belong to this laboratory".into(),
        ));
    }

    Ok(Some(parent))
}

/// Resolves the parent an existing location is moved under, rejecting the moves
/// that would detach the subtree from the tree by making it its own ancestor.
pub(super) async fn resolve_moved_parent(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &LocationRow,
    parent_location_id: Option<LocationId>,
) -> Result<Option<LocationRow>, LocationDatabaseError> {
    let Some(parent_location_id) = parent_location_id else {
        return Ok(None);
    };
    if Uuid::from(parent_location_id) == existing.location_id {
        return Err(LocationDatabaseError::Validation(
            "Location cannot be moved under itself".into(),
        ));
    }

    let parent = fetch_location_for_update(transaction, parent_location_id)
        .await?
        .ok_or(LocationDatabaseError::Validation(
            "Parent location not found".into(),
        ))?;
    if parent.laboratory_id != existing.laboratory_id {
        return Err(LocationDatabaseError::Validation(
            "Parent location does not belong to this laboratory".into(),
        ));
    }
    if path_is_self_or_descendant(&parent.path, &existing.path) {
        return Err(LocationDatabaseError::Validation(
            "Location cannot be moved under one of its descendants".into(),
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

/// Writes the new shape of a location and, when that moved it in the tree,
/// drags every descendant along so the stored paths stay consistent.
#[allow(clippy::too_many_arguments)]
pub(super) async fn move_location(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &LocationRow,
    parent_location_id: Option<LocationId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<LocationRow, LocationDatabaseError> {
    let updated = update_location_in_database(
        transaction,
        existing.location_id,
        parent_location_id.map(Uuid::from),
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
            existing.location_id,
            &existing.path,
            &updated.path,
        )
        .await?;
    }

    Ok(updated)
}

/// Deletes a location together with everything below it.
///
/// Inventory items are not deleted with the location they sit in: their
/// `location_id` is cleared first so the delete cannot fail on the foreign key,
/// and the ids are returned so the audit entry can restore them.
pub(super) async fn delete_location_subtree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, LocationDatabaseError> {
    let cleared_inventory_item_ids =
        fetch_inventory_item_ids_in_tree(transaction, laboratory_id, root_path).await?;
    clear_inventory_item_locations_in_tree(transaction, laboratory_id, root_path).await?;
    delete_location_tree(transaction, laboratory_id, root_path).await?;

    Ok(cleared_inventory_item_ids)
}
