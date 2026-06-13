use super::model::AssetCategory;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn fetch_asset_category(
    pool: &PgPool,
    category_id: Uuid,
) -> Result<AssetCategory, ApiError> {
    sqlx::query_as::<_, AssetCategory>(
        r#"
        WITH RECURSIVE category_tree AS (
            SELECT
                category_id,
                parent_category_id,
                0 AS level,
                ARRAY[category_id] AS id_path,
                ARRAY[name] AS name_path
            FROM asset_categories
            WHERE parent_category_id IS NULL

            UNION ALL

            SELECT
                child.category_id,
                child.parent_category_id,
                category_tree.level + 1,
                category_tree.id_path || child.category_id,
                category_tree.name_path || child.name
            FROM asset_categories child
            INNER JOIN category_tree ON category_tree.category_id = child.parent_category_id
        )
        SELECT
            asset_categories.category_id,
            asset_categories.laboratory_id,
            laboratories.name AS laboratory_name,
            asset_categories.parent_category_id,
            asset_categories.name,
            asset_categories.description,
            COALESCE(category_tree.level, 0) AS level,
            COALESCE(array_to_string(category_tree.name_path, ' / '), asset_categories.name) AS path_name,
            COALESCE(
                (
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'category_id',
                            path_category.category_id,
                            'name',
                            path_category.name
                        )
                        ORDER BY path_node.ordinality
                    )
                    FROM unnest(category_tree.id_path) WITH ORDINALITY AS path_node(category_id, ordinality)
                    INNER JOIN asset_categories path_category ON path_category.category_id = path_node.category_id
                ),
                jsonb_build_array(
                    jsonb_build_object(
                        'category_id',
                        asset_categories.category_id,
                        'name',
                        asset_categories.name
                    )
                )
            ) AS path,
            (
                SELECT COUNT(*)::bigint
                FROM asset_categories children
                WHERE children.parent_category_id = asset_categories.category_id
            ) AS children_count,
            (
                SELECT COUNT(*)::bigint
                FROM assets
                WHERE assets.category_id = asset_categories.category_id
            ) AS asset_count,
            asset_categories.created_at,
            asset_categories.updated_at
        FROM asset_categories
        INNER JOIN laboratories USING (laboratory_id)
        LEFT JOIN category_tree ON category_tree.category_id = asset_categories.category_id
        WHERE asset_categories.category_id = $1
        "#,
    )
    .bind(category_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) fn resolve_target_laboratory(
    actor: &Actor,
    laboratory_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    if actor.is_owner() {
        return laboratory_id
            .ok_or_else(|| ApiError::BadRequest("laboratory_id is required".into()));
    }
    let actor_laboratory_id = actor.laboratory_id.ok_or(ApiError::Forbidden)?;
    if laboratory_id.is_some() && laboratory_id != Some(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    if !actor.can_write_laboratory_resource(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    Ok(actor_laboratory_id)
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_parent_category(
    pool: &PgPool,
    laboratory_id: Uuid,
    category_id: Option<Uuid>,
    parent_category_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let Some(parent_category_id) = parent_category_id else {
        return Ok(());
    };
    if category_id == Some(parent_category_id) {
        return Err(ApiError::BadRequest(
            "parent_category_id cannot be the category itself".into(),
        ));
    }

    let parent_laboratory_id: Option<Uuid> =
        sqlx::query_scalar("SELECT laboratory_id FROM asset_categories WHERE category_id = $1")
            .bind(parent_category_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    match parent_laboratory_id {
        Some(parent_laboratory_id) if parent_laboratory_id == laboratory_id => {}
        Some(_) => {
            return Err(ApiError::BadRequest(
                "parent_category_id belongs to another laboratory".into(),
            ));
        }
        None => return Err(ApiError::BadRequest("Unknown parent asset category".into())),
    }

    if let Some(category_id) = category_id {
        let creates_cycle: bool = sqlx::query_scalar(
            r#"
            WITH RECURSIVE descendants AS (
                SELECT category_id
                FROM asset_categories
                WHERE parent_category_id = $1

                UNION ALL

                SELECT child.category_id
                FROM asset_categories child
                INNER JOIN descendants ON descendants.category_id = child.parent_category_id
            )
            SELECT EXISTS(
                SELECT 1
                FROM descendants
                WHERE category_id = $2
            )
            "#,
        )
        .bind(category_id)
        .bind(parent_category_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

        if creates_cycle {
            return Err(ApiError::BadRequest(
                "parent_category_id cannot be a descendant category".into(),
            ));
        }
    }

    Ok(())
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(value)
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return ApiError::Conflict("Asset category already exists".into()),
            Some("23503") => {
                return ApiError::Conflict("Asset category is still referenced".into());
            }
            Some("23514") => return ApiError::BadRequest("Invalid asset category data".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
