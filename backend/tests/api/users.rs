use crate::helpers::{TestUser, spawn_app};

#[tokio::test]
async fn system_admin_can_create_users_in_any_laboratory() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Physics Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "physics-admin",
            "password": "password",
            "group": "lab_admin",
            "laboratory_id": laboratory_id,
            "email": "physics-admin@example.com"
        }))
        .await;

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "physics-admin");
    assert_eq!(body["group"]["name"], "lab_admin");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );
}

#[tokio::test]
async fn creating_a_user_records_an_audit_log() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Audit User Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "audited-user",
            "password": "password",
            "group": "user",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    let user_id: uuid::Uuid = body["user_id"].as_str().unwrap().parse().unwrap();

    let audit_log_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        WHERE actor_user_id = $1
          AND target_laboratory_id = $2
          AND action = 'create'
          AND resource_type = 'user'
          AND resource_id = $3
          AND details->>'username' = 'audited-user'
        "#,
    )
    .bind(app.test_user.user_id)
    .bind(laboratory_id)
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(audit_log_count, 1);
}

#[tokio::test]
async fn lab_admin_can_create_and_delete_own_lab_user_and_guest() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Materials Lab").await;
    let lab_admin = TestUser::generate_with_group("lab_admin", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "materials-user",
            "password": "password",
            "group": "user"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["group"]["name"], "user");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );

    let user_id = body["user_id"].as_str().unwrap().parse().unwrap();
    let response = app
        .patch_user(
            user_id,
            &serde_json::json!({ "email": "updated@example.com" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app.delete_user(user_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn lab_admin_cannot_create_system_admin_or_manage_other_laboratory_users() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Other Lab").await;
    let lab_admin = TestUser::generate_with_group("lab_admin", Some(own_laboratory_id));
    let other_user = TestUser::generate_with_group("user", Some(other_laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&other_user).await;
    lab_admin.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "bad-admin",
            "password": "password",
            "group": "system_admin"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .post_user(&serde_json::json!({
            "username": "other-lab-user",
            "password": "password",
            "group": "user",
            "laboratory_id": other_laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app.delete_user(other_user.user_id).await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn regular_user_cannot_manage_users() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Biology Lab").await;
    let user = TestUser::generate_with_group("user", Some(laboratory_id));
    app.store_user(&user).await;
    user.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "biology-guest",
            "password": "password",
            "group": "guest",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}
