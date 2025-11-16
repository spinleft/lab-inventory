/*
 * @Author: spinleft spinleftgit@gmail.com
 * @Date: 2025-10-19 22:01:15
 * @LastEditors: spinleft spinleftgit@gmail.com
 * @LastEditTime: 2025-10-20 01:08:51
 * @FilePath: \lab-inventory\backend\src\routes\auth\get.rs
 * @Description:
 *
 * Copyright (c) 2025 by ${git_name_email}, All Rights Reserved.
 */
use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::HttpResponse;
use actix_web::web;
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn me(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(e500)?;
    if user_id.is_none() {
        return Ok(HttpResponse::Unauthorized().finish());
    }
    let username = get_username(user_id.unwrap(), &pool).await.map_err(e500)?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "user_id": user_id.unwrap(),
        "username": username
    })))
}

#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;
    Ok(row.username)
}
