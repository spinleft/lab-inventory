use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn list_and_get_units_are_laboratory_scoped() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Unit Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Unit Other Lab").await;

    let response = app.get_units(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 401);

    app.test_user.login(&app).await;
    let own_unit = create_unit(
        &app,
        own_laboratory_id,
        "Own Meter",
        "om",
        "length",
        1.0,
        true,
    )
    .await;
    let own_unit_id = unit_id(&own_unit);
    let other_unit = create_unit(
        &app,
        other_laboratory_id,
        "Other Meter",
        "xm",
        "length",
        1.0,
        true,
    )
    .await;
    let other_unit_id = unit_id(&other_unit);

    // A system admin can browse every laboratory, but each listing stays scoped.
    assert_eq!(
        unit_codes(&app, own_laboratory_id).await,
        vec![own_unit["code"].as_str().unwrap().to_string()]
    );
    assert_eq!(
        unit_codes(&app, other_laboratory_id).await,
        vec![other_unit["code"].as_str().unwrap().to_string()]
    );

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    assert_eq!(
        unit_codes(&app, own_laboratory_id).await,
        vec![own_unit["code"].as_str().unwrap().to_string()]
    );

    let response = app.get_unit(own_unit_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["unit_id"], own_unit_id.to_string());
    assert_eq!(body["laboratory_id"], own_laboratory_id.to_string());
    assert_eq!(body["dimension"], "length");

    // Units of another laboratory are invisible.
    let response = app.get_units(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let response = app.get_unit(other_unit_id).await;
    assert_eq!(response.status().as_u16(), 404);
    let response = app.get_unit(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 404);

    // Guests may read their own laboratory's units.
    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;

    assert_eq!(
        unit_codes(&app, own_laboratory_id).await,
        vec![own_unit["code"].as_str().unwrap().to_string()]
    );
    let response = app.get_unit(own_unit_id).await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn create_unit_allows_laboratory_writers_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Create Lab").await;
    let code = unique_unit_code();

    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": code,
                "name": "Inch",
                "symbol": "in",
                "dimension": "length",
                "scale_to_base": 0.0254,
                "allow_decimal": true
            }),
        )
        .await;

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    let unit_id: Uuid = body["unit_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(body["laboratory_id"], laboratory_id.to_string());
    assert_eq!(body["code"], code);
    assert_eq!(body["name"], "Inch");
    assert_eq!(body["symbol"], "in");
    assert_eq!(body["dimension"], "length");
    assert_eq!(body["scale_to_base"].as_f64().unwrap(), 0.0254);
    assert_eq!(body["allow_decimal"], true);

    let audit_details = latest_audit_details(&app, app.test_user.user_id, unit_id, "create").await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "unit");
    assert_eq!(
        audit_details["rollback"]["where"]["unit_id"],
        unit_id.to_string()
    );

    // A laboratory admin may create units inside their own laboratory.
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": unique_unit_code(),
                "name": "Lab Admin Unit",
                "symbol": "lau",
                "dimension": "length",
                "scale_to_base": 1,
                "allow_decimal": true
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
}

#[tokio::test]
async fn create_unit_rejects_invalid_input_and_duplicates_within_a_laboratory() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Validation Lab").await;
    let other_laboratory_id = app.create_laboratory("Unit Validation Other Lab").await;

    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": "Inch",
                "name": "Inch",
                "symbol": "in",
                "dimension": "length",
                "scale_to_base": 0.0254,
                "allow_decimal": true
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": unique_unit_code(),
                "name": "Unknown Dimension Unit",
                "symbol": "udu",
                "dimension": "unknown_dimension",
                "scale_to_base": 1,
                "allow_decimal": true
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": unique_unit_code(),
                "name": "Bad Scale",
                "symbol": "bad",
                "dimension": "length",
                "scale_to_base": 0,
                "allow_decimal": true
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let body = serde_json::json!({
        "code": unique_unit_code(),
        "name": "Custom Length",
        "symbol": "cl",
        "dimension": "length",
        "scale_to_base": 0.1,
        "allow_decimal": true
    });
    assert_eq!(
        app.post_unit(laboratory_id, &body).await.status().as_u16(),
        201
    );
    // The same code is a conflict inside the laboratory, but free in another one.
    assert_eq!(
        app.post_unit(laboratory_id, &body).await.status().as_u16(),
        409
    );
    assert_eq!(
        app.post_unit(other_laboratory_id, &body)
            .await
            .status()
            .as_u16(),
        201
    );
}

#[tokio::test]
async fn create_unit_rejects_guests_and_members_of_other_laboratories() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Forbidden Lab").await;
    let other_laboratory_id = app.create_laboratory("Unit Forbidden Other Lab").await;

    let body = serde_json::json!({
        "code": unique_unit_code(),
        "name": "Forbidden Unit",
        "symbol": "fu",
        "dimension": "length",
        "scale_to_base": 1,
        "allow_decimal": true
    });

    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    assert_eq!(
        app.post_unit(laboratory_id, &body).await.status().as_u16(),
        403
    );

    let outsider = TestUser::generate_with_user_type("lab_admin", Some(other_laboratory_id));
    app.store_user(&outsider).await;
    outsider.login(&app).await;
    assert_eq!(
        app.post_unit(laboratory_id, &body).await.status().as_u16(),
        201
    );
}

#[tokio::test]
async fn update_unit_applies_partial_changes_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Update Lab").await;
    let unit = create_unit(
        &app,
        laboratory_id,
        "Update Unit",
        "uu",
        "length",
        0.01,
        true,
    )
    .await;
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
    assert_eq!(body["laboratory_id"], laboratory_id.to_string());
    assert_eq!(body["code"], new_code);
    assert_eq!(body["name"], "Updated Unit");
    assert_eq!(body["symbol"], "upd");
    assert_eq!(body["dimension"], "length");
    assert_eq!(body["scale_to_base"].as_f64().unwrap(), 0.001);
    assert_eq!(body["allow_decimal"], false);

    let audit_details = latest_audit_details(&app, app.test_user.user_id, unit_id, "update").await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(audit_details["rollback"]["values"]["code"], unit["code"]);
    assert_eq!(audit_details["rollback"]["values"]["name"], "Update Unit");
    assert_eq!(audit_details["rollback"]["values"]["symbol"], "uu");
    assert_eq!(audit_details["rollback"]["values"]["scale_to_base"], 0.01);
}

#[tokio::test]
async fn update_unit_rejects_invalid_duplicate_and_unauthorized_requests() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Patch Lab").await;
    let unit = create_unit(
        &app,
        laboratory_id,
        "Patch Unit",
        "pu",
        "length",
        0.01,
        true,
    )
    .await;
    let unit_id = unit_id(&unit);
    let sibling = create_unit(
        &app,
        laboratory_id,
        "Patch Sibling",
        "ps",
        "length",
        0.1,
        true,
    )
    .await;

    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "scale_to_base": -1 }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "code": sibling["code"] }))
        .await;
    assert_eq!(response.status().as_u16(), 409);

    // Unknown units are indistinguishable from units the actor may not touch.
    let response = app
        .patch_unit(Uuid::new_v4(), &serde_json::json!({ "name": "Missing" }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "name": "Forbidden" }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let other_laboratory_id = app.create_laboratory("Unit Patch Other Lab").await;
    let outsider = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&outsider).await;
    outsider.login(&app).await;
    let response = app
        .patch_unit(unit_id, &serde_json::json!({ "name": "Forbidden" }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn delete_unit_removes_unreferenced_units_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Delete Lab").await;
    let unreferenced = create_unit(
        &app,
        laboratory_id,
        "Delete Unit",
        "du",
        "length",
        0.01,
        true,
    )
    .await;
    let unreferenced_unit_id = unit_id(&unreferenced);

    let response = app.delete_unit(unreferenced_unit_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let unit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM units WHERE unit_id = $1")
        .bind(unreferenced_unit_id)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(unit_count, 0);

    let audit_details =
        latest_audit_details(&app, app.test_user.user_id, unreferenced_unit_id, "delete").await;
    assert_eq!(audit_details["rollback"]["operation"], "create");
    assert_eq!(
        audit_details["rollback"]["values"]["unit_id"],
        unreferenced_unit_id.to_string()
    );
    assert_eq!(
        audit_details["rollback"]["values"]["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(audit_details["rollback"]["values"]["name"], "Delete Unit");

    let referenced = create_unit(
        &app,
        laboratory_id,
        "Referenced Unit",
        "ru",
        "count",
        1.0,
        false,
    )
    .await;
    let referenced_unit_id = unit_id(&referenced);
    insert_test_asset(&app, laboratory_id, referenced_unit_id).await;

    let response = app.delete_unit(referenced_unit_id).await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app.delete_unit(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn delete_unit_rejects_guests_and_members_of_other_laboratories() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Unit Delete Forbidden Lab").await;
    let other_laboratory_id = app
        .create_laboratory("Unit Delete Forbidden Other Lab")
        .await;
    let unit = create_unit(
        &app,
        laboratory_id,
        "Protected Unit",
        "pru",
        "length",
        0.01,
        true,
    )
    .await;
    let unit_id = unit_id(&unit);

    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    assert_eq!(app.delete_unit(unit_id).await.status().as_u16(), 404);

    let outsider = TestUser::generate_with_user_type("lab_admin", Some(other_laboratory_id));
    app.store_user(&outsider).await;
    outsider.login(&app).await;
    assert_eq!(app.delete_unit(unit_id).await.status().as_u16(), 404);
}

fn unique_unit_code() -> String {
    format!("u{}", Uuid::new_v4().simple())
}

async fn create_unit(
    app: &TestApp,
    laboratory_id: Uuid,
    name: &str,
    symbol: &str,
    dimension: &str,
    scale_to_base: f64,
    allow_decimal: bool,
) -> serde_json::Value {
    let response = app
        .post_unit(
            laboratory_id,
            &serde_json::json!({
                "code": unique_unit_code(),
                "name": name,
                "symbol": symbol,
                "dimension": dimension,
                "scale_to_base": scale_to_base,
                "allow_decimal": allow_decimal
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

fn unit_id(unit: &serde_json::Value) -> Uuid {
    unit["unit_id"].as_str().unwrap().parse().unwrap()
}

async fn unit_codes(app: &TestApp, laboratory_id: Uuid) -> Vec<String> {
    let response = app.get_units(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    body.as_array()
        .unwrap()
        .iter()
        .map(|unit| unit["code"].as_str().unwrap().to_string())
        .collect()
}

async fn insert_test_asset(app: &TestApp, laboratory_id: Uuid, unit_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            tracking_mode,
            name,
            inventory_unit_id
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

async fn latest_audit_details(
    app: &TestApp,
    actor_user_id: Uuid,
    resource_id: Uuid,
    action: &str,
) -> serde_json::Value {
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
    .bind(actor_user_id)
    .bind(action)
    .bind(resource_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
