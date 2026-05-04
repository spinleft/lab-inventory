use crate::helpers::{TestUser, spawn_app};

#[tokio::test]
async fn owner_can_manage_laboratories() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Chemistry Lab",
            "address": "Building A",
            "description": "Wet lab",
            "contact": "chem@example.com"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let laboratory_id = created["laboratory_id"].as_str().unwrap();
    assert_eq!(created["name"], "Chemistry Lab");

    let response = app.get_laboratories().await;
    assert_eq!(response.status().as_u16(), 200);
    let list: serde_json::Value = response.json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    let response = app.get_laboratory(laboratory_id.parse().unwrap()).await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app
        .patch_laboratory(
            laboratory_id.parse().unwrap(),
            &serde_json::json!({ "address": "Building B" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["address"], "Building B");

    let response = app.delete_laboratory(laboratory_id.parse().unwrap()).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn creating_a_laboratory_records_an_audit_log() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Audit Lab",
            "address": "Building A"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let laboratory_id: uuid::Uuid = created["laboratory_id"].as_str().unwrap().parse().unwrap();

    let audit_log_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        WHERE actor_user_id = $1
          AND target_laboratory_id = $2
          AND action = 'create'
          AND resource_type = 'laboratory'
          AND resource_id = $2
          AND details->>'name' = 'Audit Lab'
        "#,
    )
    .bind(app.test_user.user_id)
    .bind(laboratory_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(audit_log_count, 1);
}

#[tokio::test]
async fn duplicate_laboratory_name_returns_409() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let body = serde_json::json!({
        "name": "Physics Lab",
        "address": "Building A"
    });
    assert_eq!(app.post_laboratory(&body).await.status().as_u16(), 201);
    let response = app.post_laboratory(&body).await;
    assert_eq!(response.status().as_u16(), 409);

    let audit_log_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        WHERE action = 'create'
          AND resource_type = 'laboratory'
          AND details->>'name' = 'Physics Lab'
        "#,
    )
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(audit_log_count, 1);
}

#[tokio::test]
async fn laboratories_write_requires_owner() {
    let app = spawn_app().await;
    let lab_id = app.create_laboratory("Chemistry Lab").await;
    let lab_user = TestUser::generate_with_user_type("user", Some(lab_id));
    app.store_user(&lab_user).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Forbidden Lab",
            "address": "Building Z"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 401);

    lab_user.login(&app).await;
    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Forbidden Lab",
            "address": "Building Z"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}
