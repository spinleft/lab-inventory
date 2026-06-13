use super::model::AssetCategory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AssetCategoryListQuery {
    laboratory_id: Option<Uuid>,
    parent_category_id: Option<Uuid>,
    top_level: Option<bool>,
    cascade: Option<bool>,
}

#[tracing::instrument(name = "List asset categories", skip(pool), fields(user_id=%user_id))]
pub async fn list_asset_categories(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AssetCategoryListQuery>,
) -> Result<HttpResponse, ApiError> {
    let _actor = get_actor(pool.get_ref(), user_id).await?;
    let mut builder = QueryBuilder::<Postgres>::new(
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
        "#,
    );
    push_asset_category_filters(&mut builder, &query);
    builder.push(" ORDER BY laboratories.name, category_tree.name_path");
    let categories = builder
        .build_query_as::<AssetCategory>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(categories))
}

fn push_asset_category_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &AssetCategoryListQuery,
) {
    builder.push(" WHERE TRUE");
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND asset_categories.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }

    if let Some(parent_category_id) = query.parent_category_id {
        if query.cascade.unwrap_or(false) {
            builder.push(
                r#"
                AND asset_categories.category_id IN (
                    WITH RECURSIVE descendants AS (
                        SELECT category_id
                        FROM asset_categories
                        WHERE category_id =
                "#,
            );
            builder.push_bind(parent_category_id);
            builder.push(
                r#"
                        UNION ALL

                        SELECT child.category_id
                        FROM asset_categories child
                        INNER JOIN descendants ON descendants.category_id = child.parent_category_id
                    )
                    SELECT category_id
                    FROM descendants
                    WHERE category_id <>
                "#,
            );
            builder.push_bind(parent_category_id);
            builder.push(")");
        } else {
            builder.push(" AND asset_categories.parent_category_id = ");
            builder.push_bind(parent_category_id);
        }
    } else if query.top_level.unwrap_or(false) {
        builder.push(" AND asset_categories.parent_category_id IS NULL");
    }
}
