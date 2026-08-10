//! Every SQL statement the location routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only [`LocationDatabaseError`],
//!   so any handler can reuse them
use super::model::LocationRow;
use crate::domain::{LaboratoryId, LocationId};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum LocationDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for LocationDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> LocationDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("uq_locations_sibling_name")) => {
                return LocationDatabaseError::Conflict(
                    "Location name already exists under this parent".into(),
                );
            }
            (Some("23505"), Some("uq_locations_sibling_code")) => {
                return LocationDatabaseError::Conflict(
                    "Location code already exists under this parent".into(),
                );
            }
            (Some("23505"), Some("uq_locations_path")) => {
                return LocationDatabaseError::Conflict("Location path already exists".into());
            }
            (Some("23505"), _) => {
                return LocationDatabaseError::Conflict("Location already exists".into());
            }
            (Some("23503"), _) => {
                return LocationDatabaseError::Validation("Invalid laboratory".into());
            }
            _ => {}
        }
    }

    LocationDatabaseError::Unexpected(error.into())
}

/// A location that other rows still point at cannot be removed. That is a
/// conflict rather than bad input, so the delete path maps the foreign key
/// violation differently from [`map_database_error`].
fn map_delete_error(error: sqlx::Error) -> LocationDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return LocationDatabaseError::Conflict("Location is referenced by other records".into());
    }

    map_database_error(error)
}

// ---------------------------------------------------------------------------
// locations
// ---------------------------------------------------------------------------

pub(super) async fn fetch_location(
    pool: &PgPool,
    location_id: LocationId,
) -> Result<Option<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE location_id = $1
        "#,
        Uuid::from(location_id),
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch location")
}

/// Same projection as [`fetch_location`], but takes the row lock the write paths
/// need. `query_as!` requires a literal, so the column list cannot be shared.
pub(super) async fn fetch_location_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    location_id: LocationId,
) -> Result<Option<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE location_id = $1
        FOR UPDATE
        "#,
        Uuid::from(location_id),
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch location for update")
}

pub(super) async fn fetch_locations(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    root_path: Option<&str>,
) -> Result<Vec<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch locations")
}

pub(super) async fn fetch_location_tree_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
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
    .context("Failed to fetch location tree for update")
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Saving new location in the database",
    skip(transaction, name, code, path, description),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_location(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_location_id: Option<LocationId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<LocationRow, LocationDatabaseError> {
    sqlx::query_as!(
        LocationRow,
        r#"
        INSERT INTO locations (
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path,
            depth,
            description
        )
        VALUES ($1, $2, $3, $4, $5, $6::text::ltree, $7, $8)
        RETURNING
            location_id,
            laboratory_id,
            parent_location_id,
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
        parent_location_id.map(Uuid::from),
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
    name = "Updating location in the database",
    skip(transaction, name, code, path, description),
    fields(location_id=%location_id)
)]
pub(super) async fn update_location_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    location_id: Uuid,
    parent_location_id: Option<Uuid>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<LocationRow, LocationDatabaseError> {
    sqlx::query_as!(
        LocationRow,
        r#"
        UPDATE locations
        SET
            parent_location_id = $2,
            name = $3,
            code = $4,
            path = $5::text::ltree,
            depth = $6,
            description = $7,
            updated_at = now()
        WHERE location_id = $1
        RETURNING
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        "#,
        location_id,
        parent_location_id,
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
    name = "Updating location descendant paths in the database",
    skip(transaction, old_path, new_path),
    fields(laboratory_id=%laboratory_id, location_id=%location_id)
)]
pub(super) async fn update_descendant_paths(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    location_id: Uuid,
    old_path: &str,
    new_path: &str,
) -> Result<(), LocationDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE locations
        SET
            path = ($2::text::ltree || subpath(path, nlevel($3::text::ltree))),
            depth = nlevel($2::text::ltree || subpath(path, nlevel($3::text::ltree))) - 1,
            updated_at = now()
        WHERE laboratory_id = $1
          AND path <@ $3::text::ltree
          AND location_id <> $4
        "#,
        laboratory_id,
        new_path,
        old_path,
        location_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

#[tracing::instrument(
    name = "Deleting location tree from the database",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn delete_location_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), LocationDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM locations
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
// inventory items stored in a location
// ---------------------------------------------------------------------------

pub(super) async fn fetch_inventory_item_ids_in_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, LocationDatabaseError> {
    sqlx::query_scalar!(
        r#"
        SELECT inventory_item_id
        FROM asset_inventory_items
        WHERE laboratory_id = $1
          AND location_id IN (
              SELECT location_id
              FROM locations
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        ORDER BY inventory_item_id
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn clear_inventory_item_locations_in_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), LocationDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET location_id = NULL,
            updated_at = now()
        WHERE laboratory_id = $1
          AND location_id IN (
              SELECT location_id
              FROM locations
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
