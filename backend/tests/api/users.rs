use crate::helpers::{TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_user_allows_super_admin_to_create_scoped_users() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Users Create Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "lab-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": laboratory_id,
            "email": "lab-user@example.com",
            "phone_number": "12345678901"
        }))
        .await;

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "lab-user");
    assert_eq!(body["user_type"]["name"], "user");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(body["email"], "lab-user@example.com");
    assert_eq!(body["phone_number"], "12345678901");
}

#[tokio::test]
async fn create_user_requires_a_valid_manageable_role_and_laboratory() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Users Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Users Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "own-lab-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": own_laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app
        .post_user(&serde_json::json!({
            "username": "missing-lab-user",
            "password": "password",
            "user_type": "user"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_user(&serde_json::json!({
            "username": "other-lab-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": other_laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .post_user(&serde_json::json!({
            "username": "forbidden-super-admin",
            "password": "password",
            "user_type": "super_admin"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn create_user_rejects_duplicate_identity_fields() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Users Duplicate Lab").await;

    let body = serde_json::json!({
        "username": "duplicate-user",
        "password": "password",
        "user_type": "user",
        "laboratory_id": laboratory_id,
        "email": "duplicate@example.com"
    });

    assert_eq!(app.post_user(&body).await.status().as_u16(), 201);
    assert_eq!(app.post_user(&body).await.status().as_u16(), 409);
}

#[tokio::test]
async fn list_users_filters_results_for_lab_admins() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Users List Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Users List Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    let own_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    let other_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&own_user).await;
    app.store_user(&other_user).await;
    lab_admin.login(&app).await;

    let response = app.get_api_path("/users").await;
    assert_eq!(response.status().as_u16(), 200);
    let users: serde_json::Value = response.json().await.unwrap();
    let users = users.as_array().unwrap();

    assert!(
        users
            .iter()
            .any(|user| user["user_id"] == lab_admin.user_id.to_string())
    );
    assert!(
        users
            .iter()
            .any(|user| user["user_id"] == own_user.user_id.to_string())
    );
    assert!(
        !users
            .iter()
            .any(|user| user["user_id"] == other_user.user_id.to_string())
    );
    assert!(users.iter().all(|user| {
        user["laboratory"].is_object()
            && user["laboratory"]["laboratory_id"] == own_laboratory_id.to_string()
    }));
}

#[tokio::test]
async fn get_user_enforces_view_permissions() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Users Get Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Users Get Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    let own_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    let other_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&own_user).await;
    app.store_user(&other_user).await;
    lab_admin.login(&app).await;

    let response = app
        .get_api_path(&format!("/users/{}", own_user.user_id))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user_id"], own_user.user_id.to_string());

    let response = app
        .get_api_path(&format!("/users/{}", other_user.user_id))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .get_api_path(&format!("/users/{}", app.test_user.user_id))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn update_user_applies_nullable_profile_and_scope_changes() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let source_laboratory_id = app.create_laboratory("Users Update Source Lab").await;
    let target_laboratory_id = app.create_laboratory("Users Update Target Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "updatable-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": source_laboratory_id,
            "email": "updatable@example.com",
            "phone_number": "12345678902"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let user_id: Uuid = created["user_id"].as_str().unwrap().parse().unwrap();

    let response = app
        .patch_user(
            user_id,
            &serde_json::json!({
                "username": "updated-user",
                "user_type": "lab_admin",
                "laboratory_id": target_laboratory_id,
                "email": null,
                "phone_number": "12345678903"
            }),
        )
        .await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "updated-user");
    assert_eq!(body["user_type"]["name"], "lab_admin");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        target_laboratory_id.to_string()
    );
    assert!(body["email"].is_null());
    assert_eq!(body["phone_number"], "12345678903");

    let audit_details = latest_audit_details(&app, user_id, "update", "user").await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(
        audit_details["rollback"]["where"]["user_id"],
        user_id.to_string()
    );
    assert_eq!(
        audit_details["rollback"]["values"]["username"],
        "updatable-user"
    );
    assert_eq!(audit_details["rollback"]["values"]["user_type"], "user");
    assert_eq!(
        audit_details["rollback"]["values"]["laboratory_id"],
        source_laboratory_id.to_string()
    );
    assert_eq!(
        audit_details["rollback"]["values"]["email"],
        "updatable@example.com"
    );
    assert_eq!(
        audit_details["rollback"]["values"]["phone_number"],
        "12345678902"
    );
}

#[tokio::test]
async fn update_user_rejects_forbidden_or_invalid_changes() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Users Update Forbidden Lab").await;
    let other_laboratory_id = app.create_laboratory("Users Update Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    let own_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    let other_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&own_user).await;
    app.store_user(&other_user).await;
    lab_admin.login(&app).await;

    let response = app
        .patch_user(
            own_user.user_id,
            &serde_json::json!({ "user_type": "super_admin" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .patch_user(
            other_user.user_id,
            &serde_json::json!({ "email": "forbidden@example.com" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .patch_user(
            lab_admin.user_id,
            &serde_json::json!({ "user_type": "user" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_user(
            own_user.user_id,
            &serde_json::json!({ "password": "new-password" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn delete_user_removes_manageable_users_and_records_rollback_details() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Users Delete Lab").await;
    let target = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&target).await;
    let password_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE user_id = $1")
            .bind(target.user_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();

    let response = app.delete_user(target.user_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE user_id = $1")
        .bind(target.user_id)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(user_count, 0);

    let audit_details = latest_audit_details(&app, target.user_id, "delete", "user").await;
    assert_eq!(audit_details["rollback"]["operation"], "create");
    assert_eq!(
        audit_details["rollback"]["values"]["user_id"],
        target.user_id.to_string()
    );
    assert_eq!(
        audit_details["rollback"]["values"]["username"],
        target.username
    );
    assert_eq!(
        audit_details["rollback"]["values"]["password_hash"],
        password_hash
    );
    assert_eq!(audit_details["rollback"]["values"]["user_type"], "user");
    assert_eq!(
        audit_details["rollback"]["values"]["laboratory_id"],
        laboratory_id.to_string()
    );
}

#[tokio::test]
async fn delete_user_rejects_self_and_out_of_scope_targets() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Users Delete Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Users Delete Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    let other_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&other_user).await;
    lab_admin.login(&app).await;

    let response = app.delete_user(lab_admin.user_id).await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app.delete_user(other_user.user_id).await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn create_user_records_rollback_delete_details() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Users Create Audit Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "audited-created-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    let user_id: Uuid = body["user_id"].as_str().unwrap().parse().unwrap();

    let audit_details = latest_audit_details(&app, user_id, "create", "user").await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "user");
    assert_eq!(
        audit_details["rollback"]["where"]["user_id"],
        user_id.to_string()
    );
}

async fn latest_audit_details(
    app: &crate::helpers::TestApp,
    resource_id: Uuid,
    action: &str,
    resource_type: &str,
) -> serde_json::Value {
    sqlx::query_scalar(
        r#"
        SELECT details
        FROM audit_logs
        WHERE actor_user_id = $1
          AND action = $2
          AND resource_type = $3
          AND resource_id = $4
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(app.test_user.user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
