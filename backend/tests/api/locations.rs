use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_location_generates_paths_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Location Create Lab").await;

    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "name": "Room A",
                "code": "room_a",
                "description": "Main lab room"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let root: serde_json::Value = response.json().await.unwrap();
    let root_location_id: Uuid = root["location_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(root["laboratory_id"], laboratory_id.to_string());
    assert!(root["parent_location_id"].is_null());
    assert_eq!(root["name"], "Room A");
    assert_eq!(root["code"], "room_a");
    assert_eq!(root["path"], "room_a");
    assert_eq!(root["depth"], 0);

    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "parent_location_id": root_location_id,
                "name": "Freezer",
                "code": "freezer"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let child: serde_json::Value = response.json().await.unwrap();
    assert_eq!(child["parent_location_id"], root_location_id.to_string());
    assert_eq!(child["path"], "room_a.freezer");
    assert_eq!(child["depth"], 1);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        root_location_id,
        "create",
        "location",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "location");
    assert_eq!(
        audit_details["rollback"]["where"]["location_id"],
        root_location_id.to_string()
    );
}

#[tokio::test]
async fn list_and_get_locations_are_laboratory_scoped() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Location Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Location Other Lab").await;
    app.test_user.login(&app).await;

    let own_root = create_location(&app, own_laboratory_id, None, "Room A", "room_a").await;
    let own_root_id = location_id(&own_root);
    let own_child = create_location(
        &app,
        own_laboratory_id,
        Some(own_root_id),
        "Freezer",
        "freezer",
    )
    .await;
    let own_sibling = create_location(&app, own_laboratory_id, None, "Room B", "room_b").await;
    let other_root = create_location(&app, other_laboratory_id, None, "Other Room", "room_a").await;

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.get_locations(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let paths = location_paths(&body);
    assert_eq!(paths, vec!["room_a", "room_a.freezer", "room_b"]);

    let response = app
        .get_locations_under(own_laboratory_id, own_root_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let paths = location_paths(&body);
    assert_eq!(paths, vec!["room_a", "room_a.freezer"]);
    assert!(!paths.contains(&own_sibling["path"].as_str().unwrap()));

    let response = app.get_location(location_id(&own_child)).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["location_id"], location_id(&own_child).to_string());

    let response = app.get_locations(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(location_paths(&body), vec!["room_a"]);

    let response = app.get_location(location_id(&other_root)).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["location_id"], location_id(&other_root).to_string());
}

#[tokio::test]
async fn create_location_rejects_invalid_or_conflicting_input() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Location Validation Lab").await;
    let other_laboratory_id = app.create_laboratory("Location Validation Other Lab").await;
    let other_parent =
        create_location(&app, other_laboratory_id, None, "Other Parent", "parent").await;

    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "name": "Invalid Code",
                "code": "InvalidCode"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let body = serde_json::json!({
        "name": "Room A",
        "code": "room_a"
    });
    assert_eq!(
        app.post_location(laboratory_id, &body)
            .await
            .status()
            .as_u16(),
        201
    );
    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "name": "Room A",
                "code": "room_a_duplicate"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "name": "Room A Duplicate Code",
                "code": "room_a"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "parent_location_id": location_id(&other_parent),
                "name": "Cross Lab Child",
                "code": "cross_lab_child"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn update_location_moves_subtrees_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Location Update Lab").await;
    let room_a = create_location(&app, laboratory_id, None, "Room A", "room_a").await;
    let room_b = create_location(&app, laboratory_id, None, "Room B", "room_b").await;
    let cabinet = create_location(
        &app,
        laboratory_id,
        Some(location_id(&room_a)),
        "Cabinet",
        "cabinet",
    )
    .await;
    let shelf = create_location(
        &app,
        laboratory_id,
        Some(location_id(&cabinet)),
        "Shelf",
        "shelf",
    )
    .await;
    let cabinet_id = location_id(&cabinet);
    let shelf_id = location_id(&shelf);

    let response = app
        .patch_location(
            cabinet_id,
            &serde_json::json!({
                "parent_location_id": location_id(&room_b),
                "name": "Cabinets",
                "code": "cabinets",
                "description": null
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["path"], "room_b.cabinets");
    assert_eq!(updated["depth"], 1);
    assert_eq!(
        updated["parent_location_id"],
        location_id(&room_b).to_string()
    );
    assert!(updated["description"].is_null());

    let response = app.get_location(shelf_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let moved_child: serde_json::Value = response.json().await.unwrap();
    assert_eq!(moved_child["path"], "room_b.cabinets.shelf");
    assert_eq!(moved_child["depth"], 2);

    let response = app
        .get_locations_under(laboratory_id, location_id(&room_b))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        location_paths(&body),
        vec!["room_b", "room_b.cabinets", "room_b.cabinets.shelf"]
    );

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        cabinet_id,
        "update",
        "location",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(
        audit_details["rollback"]["values"]["path"],
        "room_a.cabinet"
    );
    assert_eq!(audit_details["rollback"]["values"]["code"], "cabinet");
}

#[tokio::test]
async fn update_location_rejects_self_or_descendant_parent() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Location Invalid Move Lab").await;
    let root = create_location(&app, laboratory_id, None, "Root", "root").await;
    let child = create_location(
        &app,
        laboratory_id,
        Some(location_id(&root)),
        "Child",
        "child",
    )
    .await;

    let response = app
        .patch_location(
            location_id(&root),
            &serde_json::json!({ "parent_location_id": location_id(&root) }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_location(
            location_id(&root),
            &serde_json::json!({ "parent_location_id": location_id(&child) }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn delete_location_deletes_tree_clears_inventory_items_and_records_audit() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Location Delete Lab").await;
    let root = create_location(&app, laboratory_id, None, "Room A", "room_a").await;
    let child = create_location(
        &app,
        laboratory_id,
        Some(location_id(&root)),
        "Cabinet",
        "cabinet",
    )
    .await;
    let root_id = location_id(&root);
    let child_id = location_id(&child);
    let inventory_item_id = insert_test_inventory_item(&app, laboratory_id, child_id).await;

    let response = app.delete_location(root_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let location_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM locations WHERE location_id IN ($1, $2)")
            .bind(root_id)
            .bind(child_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(location_count, 0);

    let item_location_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT location_id FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(inventory_item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(item_location_id, None);

    let audit_details =
        latest_audit_details(&app, app.test_user.user_id, root_id, "delete", "location").await;
    assert_eq!(audit_details["rollback"]["operation"], "restore_tree");
    assert_eq!(
        audit_details["rollback"]["values"]["locations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let inventory_item_id = inventory_item_id.to_string();
    assert!(
        audit_details["rollback"]["values"]["cleared_inventory_item_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some(inventory_item_id.as_str()))
    );
}

#[tokio::test]
async fn write_permissions_follow_laboratory_scope() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Location Permission Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Location Permission Other Lab").await;

    let response = app
        .post_location(
            own_laboratory_id,
            &serde_json::json!({ "name": "Unauthenticated", "code": "unauthenticated" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .post_location(
            own_laboratory_id,
            &serde_json::json!({ "name": "Guest", "code": "guest" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;
    let response = app
        .post_location(
            own_laboratory_id,
            &serde_json::json!({ "name": "Own Lab", "code": "own_lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app
        .post_location(
            other_laboratory_id,
            &serde_json::json!({ "name": "Other Lab", "code": "other_lab" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app
        .post_location(
            other_laboratory_id,
            &serde_json::json!({ "name": "Server Admin", "code": "server_admin" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
}

async fn create_location(
    app: &TestApp,
    laboratory_id: Uuid,
    parent_location_id: Option<Uuid>,
    name: &str,
    code: &str,
) -> serde_json::Value {
    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "parent_location_id": parent_location_id,
                "name": name,
                "code": code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

fn location_id(location: &serde_json::Value) -> Uuid {
    location["location_id"].as_str().unwrap().parse().unwrap()
}

fn location_paths(body: &serde_json::Value) -> Vec<&str> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|location| location["path"].as_str().unwrap())
        .collect()
}

async fn insert_test_inventory_item(app: &TestApp, laboratory_id: Uuid, location_id: Uuid) -> Uuid {
    let unit_id = app.unit_id("pcs").await;
    let asset_id: Uuid = sqlx::query_scalar(
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
    .unwrap();

    sqlx::query_scalar(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id,
            location_id
        )
        VALUES ($1, $2, $3, 'quantity', 1, 0, $4, $5)
        RETURNING inventory_item_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .bind(location_id)
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
