use crate::helpers::{TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_laboratory_allows_super_admin_and_lab_admin_users() {
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
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["laboratory_id"].as_str().is_some());
    assert_eq!(body["name"], "Chemistry Lab");
    assert_eq!(body["address"], "Building A");
    assert_eq!(body["description"], "Wet lab");
    assert_eq!(body["contact"], "chem@example.com");

    let laboratory_id = app.create_laboratory("Lab Admin Source Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Lab Admin Created Lab",
            "address": "Building B"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
}

#[tokio::test]
async fn create_laboratory_validates_required_fields_and_unique_names() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "   ",
            "address": "Building A"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let body = serde_json::json!({
        "name": "Unique Lab",
        "address": "Building A"
    });
    assert_eq!(app.post_laboratory(&body).await.status().as_u16(), 201);
    assert_eq!(app.post_laboratory(&body).await.status().as_u16(), 409);
}

#[tokio::test]
async fn create_laboratory_rejects_non_admin_users() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Regular User Lab").await;
    let regular_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&regular_user).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Unauthenticated Lab",
            "address": "Building Z"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 401);

    regular_user.login(&app).await;
    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Forbidden Lab",
            "address": "Building Z"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn list_laboratories_returns_all_labs_for_server_admins() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let first_laboratory_id = app.create_laboratory("List Lab A").await;
    let second_laboratory_id = app.create_laboratory("List Lab B").await;

    let response = app.get_laboratories().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let laboratories = body.as_array().unwrap();

    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == first_laboratory_id.to_string())
    );
    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == second_laboratory_id.to_string())
    );
}

#[tokio::test]
async fn list_laboratories_returns_all_labs_for_lab_admins() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Lab Admin Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Lab Admin Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app.get_laboratories().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let laboratories = body.as_array().unwrap();

    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == own_laboratory_id.to_string())
    );
    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == other_laboratory_id.to_string())
    );
}

#[tokio::test]
async fn list_laboratories_returns_an_empty_list_for_unscoped_lab_admins() {
    let app = spawn_app().await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", None);
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app.get_laboratories().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn list_laboratories_returns_all_labs_for_regular_users() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Regular User Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Regular User Other Lab").await;
    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.get_laboratories().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let laboratories = body.as_array().unwrap();

    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == own_laboratory_id.to_string())
    );
    assert!(
        laboratories
            .iter()
            .any(|lab| lab["laboratory_id"] == other_laboratory_id.to_string())
    );
}

#[tokio::test]
async fn get_laboratory_enforces_scope_and_not_found() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Get Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Get Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app.get_laboratory(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["laboratory_id"], own_laboratory_id.to_string());

    let response = app.get_laboratory(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app.get_laboratory(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn update_laboratory_applies_partial_and_nullable_updates() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Update Lab",
            "address": "Building A",
            "description": "Old description",
            "contact": "old@example.com"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let laboratory_id: Uuid = created["laboratory_id"].as_str().unwrap().parse().unwrap();

    let response = app
        .patch_laboratory(
            laboratory_id,
            &serde_json::json!({
                "name": "Updated Lab",
                "address": "Building B",
                "description": null,
                "contact": null
            }),
        )
        .await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Updated Lab");
    assert_eq!(body["address"], "Building B");
    assert!(body["description"].is_null());
    assert!(body["contact"].is_null());

    let audit_details = latest_audit_details(&app, laboratory_id, "update", "laboratory").await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(
        audit_details["rollback"]["where"]["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(audit_details["rollback"]["values"]["name"], "Update Lab");
    assert_eq!(audit_details["rollback"]["values"]["address"], "Building A");
    assert_eq!(
        audit_details["rollback"]["values"]["description"],
        "Old description"
    );
    assert_eq!(
        audit_details["rollback"]["values"]["contact"],
        "old@example.com"
    );
}

#[tokio::test]
async fn update_laboratory_rejects_out_of_scope_or_invalid_changes() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Update Scope Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Update Scope Other Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .patch_laboratory(
            own_laboratory_id,
            &serde_json::json!({ "name": "Updated Own Lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app
        .patch_laboratory(
            other_laboratory_id,
            &serde_json::json!({ "name": "Forbidden Other Lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app
        .patch_laboratory(own_laboratory_id, &serde_json::json!({ "name": "   " }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_laboratory(
            Uuid::new_v4(),
            &serde_json::json!({ "name": "Missing Lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn update_laboratory_rejects_duplicate_names() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let first_laboratory_id = app.create_laboratory("Duplicate Target Lab").await;
    app.create_laboratory("Duplicate Existing Lab").await;

    let response = app
        .patch_laboratory(
            first_laboratory_id,
            &serde_json::json!({ "name": "Duplicate Existing Lab" }),
        )
        .await;

    assert_eq!(response.status().as_u16(), 409);
}

#[tokio::test]
async fn delete_laboratory_allows_server_admins_and_records_rollback_details() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Delete Lab",
            "address": "Building D",
            "description": "Temporary lab",
            "contact": "delete@example.com"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let laboratory_id: Uuid = created["laboratory_id"].as_str().unwrap().parse().unwrap();

    let response = app.delete_laboratory(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let laboratory_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM laboratories WHERE laboratory_id = $1")
            .bind(laboratory_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(laboratory_count, 0);

    let audit_details = latest_audit_details(&app, laboratory_id, "delete", "laboratory").await;
    assert_eq!(audit_details["rollback"]["operation"], "create");
    assert_eq!(
        audit_details["rollback"]["values"]["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(audit_details["rollback"]["values"]["name"], "Delete Lab");
    assert_eq!(audit_details["rollback"]["values"]["address"], "Building D");
    assert_eq!(
        audit_details["rollback"]["values"]["description"],
        "Temporary lab"
    );
    assert_eq!(
        audit_details["rollback"]["values"]["contact"],
        "delete@example.com"
    );
}

#[tokio::test]
async fn delete_laboratory_rejects_lab_admins_and_referenced_laboratories() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Delete Forbidden Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app.delete_laboratory(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app.delete_laboratory(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 409);
}

#[tokio::test]
async fn create_laboratory_records_rollback_delete_details() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_laboratory(&serde_json::json!({
            "name": "Create Audit Lab",
            "address": "Building A"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let laboratory_id: Uuid = created["laboratory_id"].as_str().unwrap().parse().unwrap();

    let audit_details = latest_audit_details(&app, laboratory_id, "create", "laboratory").await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "laboratory");
    assert_eq!(
        audit_details["rollback"]["where"]["laboratory_id"],
        laboratory_id.to_string()
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
