use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_asset_category_generates_paths_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Category Create Lab").await;

    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": "显微镜",
                "code": "microscope",
                "description": "Microscope assets"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let root: serde_json::Value = response.json().await.unwrap();
    let root_category_id: Uuid = root["category_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(root["laboratory_id"], laboratory_id.to_string());
    assert!(root["parent_category_id"].is_null());
    assert_eq!(root["name"], "显微镜");
    assert_eq!(root["code"], "microscope");
    assert_eq!(root["path"], "microscope");
    assert_eq!(root["depth"], 0);

    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "parent_category_id": root_category_id,
                "name": "光学显微镜",
                "code": "optical"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let child: serde_json::Value = response.json().await.unwrap();
    assert_eq!(child["parent_category_id"], root_category_id.to_string());
    assert_eq!(child["path"], "microscope.optical");
    assert_eq!(child["depth"], 1);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        root_category_id,
        "create",
        "asset_category",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "asset_category");
    assert_eq!(
        audit_details["rollback"]["where"]["category_id"],
        root_category_id.to_string()
    );
}

#[tokio::test]
async fn list_and_get_asset_categories_are_laboratory_scoped() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Asset Category Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Asset Category Other Lab").await;
    app.test_user.login(&app).await;

    let own_root = create_category(&app, own_laboratory_id, None, "Equipment", "equipment").await;
    let own_root_id = category_id(&own_root);
    let own_child = create_category(
        &app,
        own_laboratory_id,
        Some(own_root_id),
        "Microscopes",
        "microscopes",
    )
    .await;
    let own_sibling =
        create_category(&app, own_laboratory_id, None, "Materials", "materials").await;
    let other_root = create_category(
        &app,
        other_laboratory_id,
        None,
        "Other Equipment",
        "equipment",
    )
    .await;

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.get_asset_categories(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let paths = category_paths(&body);
    assert_eq!(
        paths,
        vec!["equipment", "equipment.microscopes", "materials"]
    );

    let response = app
        .get_asset_categories_under(own_laboratory_id, own_root_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let paths = category_paths(&body);
    assert_eq!(paths, vec!["equipment", "equipment.microscopes"]);
    assert!(!paths.contains(&own_sibling["path"].as_str().unwrap()));

    let response = app.get_asset_category(category_id(&own_child)).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["category_id"], category_id(&own_child).to_string());

    let response = app.get_asset_categories(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 403);
    let response = app.get_asset_category(category_id(&other_root)).await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn create_asset_category_rejects_invalid_or_conflicting_input() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Category Validation Lab").await;
    let other_laboratory_id = app
        .create_laboratory("Asset Category Validation Other Lab")
        .await;
    let other_parent =
        create_category(&app, other_laboratory_id, None, "Other Parent", "parent").await;

    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": "Invalid Code",
                "code": "InvalidCode"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let body = serde_json::json!({
        "name": "Equipment",
        "code": "equipment"
    });
    assert_eq!(
        app.post_asset_category(laboratory_id, &body)
            .await
            .status()
            .as_u16(),
        201
    );
    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": "Equipment",
                "code": "equipment_duplicate"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": "Equipment Duplicate Code",
                "code": "equipment"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "parent_category_id": category_id(&other_parent),
                "name": "Cross Lab Child",
                "code": "cross_lab_child"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn update_asset_category_moves_subtrees_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Category Update Lab").await;
    let equipment = create_category(&app, laboratory_id, None, "Equipment", "equipment").await;
    let materials = create_category(&app, laboratory_id, None, "Materials", "materials").await;
    let microscope = create_category(
        &app,
        laboratory_id,
        Some(category_id(&equipment)),
        "Microscope",
        "microscope",
    )
    .await;
    let optical = create_category(
        &app,
        laboratory_id,
        Some(category_id(&microscope)),
        "Optical",
        "optical",
    )
    .await;
    let microscope_id = category_id(&microscope);
    let optical_id = category_id(&optical);

    let response = app
        .patch_asset_category(
            microscope_id,
            &serde_json::json!({
                "parent_category_id": category_id(&materials),
                "name": "Microscopes",
                "code": "microscopes",
                "description": null
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["path"], "materials.microscopes");
    assert_eq!(updated["depth"], 1);
    assert_eq!(
        updated["parent_category_id"],
        category_id(&materials).to_string()
    );
    assert!(updated["description"].is_null());

    let response = app.get_asset_category(optical_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let moved_child: serde_json::Value = response.json().await.unwrap();
    assert_eq!(moved_child["path"], "materials.microscopes.optical");
    assert_eq!(moved_child["depth"], 2);

    let response = app
        .get_asset_categories_under(laboratory_id, category_id(&materials))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        category_paths(&body),
        vec![
            "materials",
            "materials.microscopes",
            "materials.microscopes.optical"
        ]
    );

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        microscope_id,
        "update",
        "asset_category",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(
        audit_details["rollback"]["values"]["path"],
        "equipment.microscope"
    );
    assert_eq!(audit_details["rollback"]["values"]["code"], "microscope");
}

#[tokio::test]
async fn update_asset_category_rejects_self_or_descendant_parent() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Category Invalid Move Lab")
        .await;
    let root = create_category(&app, laboratory_id, None, "Root", "root").await;
    let child = create_category(
        &app,
        laboratory_id,
        Some(category_id(&root)),
        "Child",
        "child",
    )
    .await;

    let response = app
        .patch_asset_category(
            category_id(&root),
            &serde_json::json!({ "parent_category_id": category_id(&root) }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_asset_category(
            category_id(&root),
            &serde_json::json!({ "parent_category_id": category_id(&child) }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn delete_asset_category_deletes_tree_clears_assets_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Category Delete Lab").await;
    let root = create_category(&app, laboratory_id, None, "Equipment", "equipment").await;
    let child = create_category(
        &app,
        laboratory_id,
        Some(category_id(&root)),
        "Microscopes",
        "microscopes",
    )
    .await;
    let root_id = category_id(&root);
    let child_id = category_id(&child);
    let asset_id = insert_test_asset(&app, laboratory_id, child_id).await;

    let response = app.delete_asset_category(root_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let category_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asset_categories WHERE category_id IN ($1, $2)")
            .bind(root_id)
            .bind(child_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(category_count, 0);

    let asset_category_id: Option<Uuid> =
        sqlx::query_scalar("SELECT category_id FROM assets WHERE asset_id = $1")
            .bind(asset_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(asset_category_id, None);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        root_id,
        "delete",
        "asset_category",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "restore_tree");
    assert_eq!(
        audit_details["rollback"]["values"]["categories"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        audit_details["rollback"]["values"]["cleared_asset_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == &serde_json::json!(asset_id))
    );
}

#[tokio::test]
async fn write_permissions_follow_laboratory_scope() {
    let app = spawn_app().await;
    let own_laboratory_id = app
        .create_laboratory("Asset Category Permission Own Lab")
        .await;
    let other_laboratory_id = app
        .create_laboratory("Asset Category Permission Other Lab")
        .await;

    let response = app
        .post_asset_category(
            own_laboratory_id,
            &serde_json::json!({ "name": "Unauthenticated", "code": "unauthenticated" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .post_asset_category(
            own_laboratory_id,
            &serde_json::json!({ "name": "Guest", "code": "guest" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;
    let response = app
        .post_asset_category(
            own_laboratory_id,
            &serde_json::json!({ "name": "Own Lab", "code": "own_lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app
        .post_asset_category(
            other_laboratory_id,
            &serde_json::json!({ "name": "Other Lab", "code": "other_lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app
        .post_asset_category(
            other_laboratory_id,
            &serde_json::json!({ "name": "Server Admin", "code": "server_admin" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
}

async fn create_category(
    app: &TestApp,
    laboratory_id: Uuid,
    parent_category_id: Option<Uuid>,
    name: &str,
    code: &str,
) -> serde_json::Value {
    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "parent_category_id": parent_category_id,
                "name": name,
                "code": code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

fn category_id(category: &serde_json::Value) -> Uuid {
    category["category_id"].as_str().unwrap().parse().unwrap()
}

fn category_paths(body: &serde_json::Value) -> Vec<&str> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|category| category["path"].as_str().unwrap())
        .collect()
}

async fn insert_test_asset(app: &TestApp, laboratory_id: Uuid, category_id: Uuid) -> Uuid {
    let unit_id = app.unit_id("pcs").await;
    sqlx::query_scalar(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            category_id,
            tracking_mode,
            name,
            default_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', $4, $5)
        RETURNING asset_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(category_id)
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
    .bind(actor_user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
