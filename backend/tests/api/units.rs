use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn list_and_get_units_are_available_to_authenticated_users() {
    let app = spawn_app().await;

    let response = app.get_units().await;
    assert_eq!(response.status().as_u16(), 401);

    let laboratory_id = app.create_laboratory("Unit Read Lab").await;
    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;

    let response = app.get_units().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|unit| unit["code"] == "mm")
    );

    let unit_id = app.unit_id("mm").await;
    let response = app.get_unit(unit_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["unit_id"], unit_id.to_string());
    assert_eq!(body["code"], "mm");
    assert_eq!(body["dimension"], "length");

    let response = app.get_unit(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn create_unit_allows_server_admins_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let code = unique_unit_code();

    let response = app
        .post_unit(&serde_json::json!({
            "code": code,
            "name": "Inch",
            "symbol": "in",
            "dimension": "length",
            "scale_to_base": 0.0254,
            "allow_decimal": true
        }))
        .await;

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    let unit_id: Uuid = body["unit_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(body["code"], code);
    assert_eq!(body["name"], "Inch");
    assert_eq!(body["symbol"], "in");
    assert_eq!(body["dimension"], "length");
    assert_eq!(body["scale_to_base"].as_f64().unwrap(), 0.0254);
    assert_eq!(body["allow_decimal"], true);

    let audit_details = latest_audit_details(&app, unit_id, "create").await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "unit");
    assert_eq!(
        audit_details["rollback"]["where"]["unit_id"],
        unit_id.to_string()
    );
}

#[tokio::test]
async fn create_unit_rejects_invalid_duplicate_and_non_server_admin_users() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .post_unit(&serde_json::json!({
            "code": "Inch",
            "name": "Inch",
            "symbol": "in",
            "dimension": "length",
            "scale_to_base": 0.0254,
            "allow_decimal": true
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_unit(&serde_json::json!({
            "code": unique_unit_code(),
            "name": "Unknown Dimension Unit",
            "symbol": "udu",
            "dimension": "unknown_dimension",
            "scale_to_base": 1,
            "allow_decimal": true
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_unit(&serde_json::json!({
            "code": unique_unit_code(),
            "name": "Bad Scale",
            "symbol": "bad",
            "dimension": "length",
            "scale_to_base": 0,
            "allow_decimal": true
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let code = unique_unit_code();
    let body = serde_json::json!({
        "code": code,
        "name": "Custom Length",
        "symbol": "cl",
        "dimension": "length",
        "scale_to_base": 0.1,
        "allow_decimal": true
    });
    assert_eq!(app.post_unit(&body).await.status().as_u16(), 201);
    assert_eq!(app.post_unit(&body).await.status().as_u16(), 409);

    let laboratory_id = app.create_laboratory("Unit Forbidden Lab").await;
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .post_unit(&serde_json::json!({
            "code": unique_unit_code(),
            "name": "Forbidden Unit",
            "symbol": "fu",
            "dimension": "length",
            "scale_to_base": 1,
            "allow_decimal": true
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn update_unit_applies_partial_changes_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let unit = create_unit(&app, "Update Unit", "uu", "length", 0.01, true).await;
    let unit_id = unit_id(&unit);
    let new_code = unique_unit_code();

    let response = app
        .patch_unit(
            unit_id,
            &serde_json::json!({
                "code": new_code,
                "name": "Updated Unit",
                "symbol": "upd",
                "scale_to_base": 0.001,
                "allow_decimal": false
            }),
        )
        .await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["code"], new_code);
    assert_eq!(body["name"], "Updated Unit");
    assert_eq!(body["symbol"], "upd");
    assert_eq!(body["dimension"], "length");
    assert_eq!(body["scale_to_base"].as_f64().unwrap(), 0.001);
    assert_eq!(body["allow_decimal"], false);

    let audit_details = latest_audit_details(&app, unit_id, "update").await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(audit_details["rollback"]["values"]["code"], unit["code"]);
    assert_eq!(audit_details["rollback"]["values"]["name"], "Update Unit");
    assert_eq!(audit_details["rollback"]["values"]["symbol"], "uu");
    assert_eq!(audit_details["rollback"]["values"]["scale_to_base"], 0.01);
}

#[tokio::test]
async fn update_unit_rejects_invalid_duplicate_forbidden_and_missing_units() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let unit = create_unit(&app, "Patch Unit", "pu", "length", 0.01, true).await;
    let unit_id = unit_id(&unit);

    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "scale_to_base": -1 }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "code": "mm" }))
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .patch_unit(Uuid::new_v4(), &serde_json::json!({ "name": "Missing" }))
        .await;
    assert_eq!(response.status().as_u16(), 404);

    let laboratory_id = app.create_laboratory("Unit Patch Forbidden Lab").await;
    let regular_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "name": "Forbidden" }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn delete_unit_allows_server_admins_and_rejects_referenced_units() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let unreferenced = create_unit(&app, "Delete Unit", "du", "length", 0.01, true).await;
    let unreferenced_unit_id = unit_id(&unreferenced);

    let response = app.delete_unit(unreferenced_unit_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let unit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM units WHERE unit_id = $1")
        .bind(unreferenced_unit_id)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(unit_count, 0);

    let audit_details = latest_audit_details(&app, unreferenced_unit_id, "delete").await;
    assert_eq!(audit_details["rollback"]["operation"], "create");
    assert_eq!(
        audit_details["rollback"]["values"]["unit_id"],
        unreferenced_unit_id.to_string()
    );
    assert_eq!(audit_details["rollback"]["values"]["name"], "Delete Unit");

    let referenced = create_unit(&app, "Referenced Unit", "ru", "count", 1.0, false).await;
    let referenced_unit_id = unit_id(&referenced);
    let laboratory_id = app.create_laboratory("Referenced Unit Lab").await;
    insert_test_asset(&app, laboratory_id, referenced_unit_id).await;

    let response = app.delete_unit(referenced_unit_id).await;
    assert_eq!(response.status().as_u16(), 409);
}

fn unique_unit_code() -> String {
    format!("u{}", Uuid::new_v4().simple())
}

async fn create_unit(
    app: &TestApp,
    name: &str,
    symbol: &str,
    dimension: &str,
    scale_to_base: f64,
    allow_decimal: bool,
) -> serde_json::Value {
    let response = app
        .post_unit(&serde_json::json!({
            "code": unique_unit_code(),
            "name": name,
            "symbol": symbol,
            "dimension": dimension,
            "scale_to_base": scale_to_base,
            "allow_decimal": allow_decimal
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

fn unit_id(unit: &serde_json::Value) -> Uuid {
    unit["unit_id"].as_str().unwrap().parse().unwrap()
}

async fn insert_test_asset(app: &TestApp, laboratory_id: Uuid, unit_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            tracking_mode,
            name,
            default_unit_id
        )
        VALUES ($1, $2, 'quantity', $3, $4)
        RETURNING asset_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(format!("Test Asset {}", Uuid::new_v4()))
    .bind(unit_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

async fn latest_audit_details(app: &TestApp, resource_id: Uuid, action: &str) -> serde_json::Value {
    sqlx::query_scalar(
        r#"
        SELECT details
        FROM audit_logs
        WHERE actor_user_id = $1
          AND action = $2
          AND resource_type = 'unit'
          AND resource_id = $3
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(app.test_user.user_id)
    .bind(action)
    .bind(resource_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
