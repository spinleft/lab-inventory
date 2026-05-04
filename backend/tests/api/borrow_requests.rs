use crate::helpers::{TestUser, spawn_app};
use uuid::Uuid;

async fn create_quantity_item(
    app: &crate::helpers::TestApp,
    laboratory_id: Uuid,
    name: &str,
    quantity: f64,
    borrowable: bool,
) -> Uuid {
    app.test_user.login(app).await;
    let meter = app.unit_id("m").await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": name,
            "default_unit_id": meter
        }))
        .await
        .json()
        .await
        .unwrap();
    let item: serde_json::Value = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": asset["asset_id"],
                "quantity_on_hand": quantity,
                "unit_id": meter,
                "is_cross_lab_borrowable": borrowable
            }),
            &format!("{name}-inventory"),
        )
        .await
        .json()
        .await
        .unwrap();
    item["inventory_item_id"].as_str().unwrap().parse().unwrap()
}

async fn create_serialized_item(app: &crate::helpers::TestApp, laboratory_id: Uuid) -> Uuid {
    app.test_user.login(app).await;
    let pcs = app.unit_id("pcs").await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Portable Analyzer",
            "default_unit_id": pcs
        }))
        .await
        .json()
        .await
        .unwrap();
    let item: serde_json::Value = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": asset["asset_id"],
                "serial_number": "PA-001",
                "is_cross_lab_borrowable": true
            }),
            "serialized-borrow-inventory",
        )
        .await
        .json()
        .await
        .unwrap();
    item["inventory_item_id"].as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn creating_borrow_request_enforces_permissions_and_availability() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Borrow Owner Lab").await;
    let requester_lab = app.create_laboratory("Borrow Requester Lab").await;
    let item_id = create_quantity_item(&app, owner_lab, "Shared Fiber", 5.0, true).await;
    let private_item_id = create_quantity_item(&app, owner_lab, "Private Fiber", 5.0, false).await;
    app.post_logout().await;

    let body = serde_json::json!({
        "inventory_item_id": item_id,
        "requested_quantity": 2.5,
        "expected_borrowed_at": "2026-05-03T00:00:00Z",
        "expected_returned_at": "2026-05-10T00:00:00Z",
        "purpose": "alignment test"
    });
    let response = app.post_borrow_request(&body, "borrow-unauth").await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_user_type("guest", Some(requester_lab));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app.post_borrow_request(&body, "borrow-guest").await;
    assert_eq!(response.status().as_u16(), 403);

    let owner_user = TestUser::generate_with_user_type("user", Some(owner_lab));
    app.store_user(&owner_user).await;
    owner_user.login(&app).await;
    let response = app.post_borrow_request(&body, "borrow-same-lab").await;
    assert_eq!(response.status().as_u16(), 400);

    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    app.store_user(&requester).await;
    requester.login(&app).await;
    let response = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 6.0,
                "purpose": "too much"
            }),
            "borrow-too-much",
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": private_item_id,
                "requested_quantity": 1.0,
                "purpose": "private stock"
            }),
            "borrow-private",
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app.post_borrow_request(&body, "borrow-create").await;
    assert_eq!(response.status().as_u16(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    assert_eq!(created["status"], "pending");
    assert_eq!(created["requested_quantity"], 2.5);

    let response = app.get_borrow_requests().await;
    assert_eq!(response.status().as_u16(), 200);
    let listed: serde_json::Value = response.json().await.unwrap();
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        listed["items"][0]["borrow_request_id"],
        created["borrow_request_id"]
    );

    let borrow_request_id: Uuid = created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let response = app.get_borrow_request(borrow_request_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let fetched: serde_json::Value = response.json().await.unwrap();
    assert_eq!(fetched["borrow_request_id"], created["borrow_request_id"]);
}

#[tokio::test]
async fn owner_lab_can_approve_and_idempotency_prevents_double_allocation() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Approve Owner Lab").await;
    let requester_lab = app.create_laboratory("Approve Requester Lab").await;
    let item_id = create_quantity_item(&app, owner_lab, "Approval Cable", 5.0, true).await;
    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    app.store_user(&requester).await;
    app.store_user(&owner).await;

    requester.login(&app).await;
    let created: serde_json::Value = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 2.0,
                "purpose": "shared measurement"
            }),
            "approve-create",
        )
        .await
        .json()
        .await
        .unwrap();
    let borrow_request_id: Uuid = created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let response = app
        .post_borrow_request_operation(
            borrow_request_id,
            "approve",
            &serde_json::json!({ "comment": "ok" }),
            "approve-key",
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    owner.login(&app).await;
    let response = app
        .post_borrow_request_operation(
            borrow_request_id,
            "approve",
            &serde_json::json!({ "comment": "ok" }),
            "approve-key",
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let approved: serde_json::Value = response.json().await.unwrap();
    assert_eq!(approved["status"], "approved");

    let response = app
        .post_borrow_request_operation(
            borrow_request_id,
            "approve",
            &serde_json::json!({ "comment": "ok" }),
            "approve-key",
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let allocated: f64 = sqlx::query_scalar(
        "SELECT quantity_allocated FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(allocated, 2.0);

    let allocation_transactions: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM inventory_transactions
        WHERE related_resource_type = 'borrow_request'
          AND related_resource_id = $1
          AND action = 'allocate'
        "#,
    )
    .bind(borrow_request_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(allocation_transactions, 1);
}

#[tokio::test]
async fn owner_lab_can_reject_pending_request_idempotently() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Reject Owner Lab").await;
    let requester_lab = app.create_laboratory("Reject Requester Lab").await;
    let item_id = create_quantity_item(&app, owner_lab, "Rejectable Lens", 3.0, true).await;
    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    app.store_user(&requester).await;
    app.store_user(&owner).await;

    requester.login(&app).await;
    let created: serde_json::Value = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 1.0,
                "purpose": "short trial"
            }),
            "reject-create",
        )
        .await
        .json()
        .await
        .unwrap();
    let borrow_request_id: Uuid = created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    owner.login(&app).await;
    let rejected: serde_json::Value = app
        .post_borrow_request_operation(
            borrow_request_id,
            "reject",
            &serde_json::json!({ "comment": "busy" }),
            "reject-key",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(rejected["status"], "rejected");

    let replayed: serde_json::Value = app
        .post_borrow_request_operation(
            borrow_request_id,
            "reject",
            &serde_json::json!({ "comment": "busy" }),
            "reject-key",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(replayed["status"], "rejected");

    let response = app
        .post_borrow_request_operation(
            borrow_request_id,
            "approve",
            &serde_json::json!({ "comment": "late" }),
            "reject-then-approve",
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let inventory_transactions: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM inventory_transactions
        WHERE related_resource_type = 'borrow_request'
          AND related_resource_id = $1
        "#,
    )
    .bind(borrow_request_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(inventory_transactions, 1);
}

#[tokio::test]
async fn serialized_borrow_out_and_return_update_status_and_release_allocation() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Serialized Owner Lab").await;
    let requester_lab = app.create_laboratory("Serialized Requester Lab").await;
    let item_id = create_serialized_item(&app, owner_lab).await;
    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    let owner = TestUser::generate_with_user_type("maintainer", Some(owner_lab));
    app.store_user(&requester).await;
    app.store_user(&owner).await;

    requester.login(&app).await;
    let created: serde_json::Value = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 1.0,
                "purpose": "calibration"
            }),
            "serialized-request",
        )
        .await
        .json()
        .await
        .unwrap();
    let borrow_request_id: Uuid = created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    owner.login(&app).await;
    app.post_borrow_request_operation(
        borrow_request_id,
        "approve",
        &serde_json::json!({ "comment": "approved" }),
        "serialized-approve",
    )
    .await;
    let inventory_state: (String, f64) = sqlx::query_as(
        "SELECT status, quantity_allocated FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(inventory_state, ("reserved".to_string(), 1.0));

    let borrowed: serde_json::Value = app
        .post_borrow_request_operation(
            borrow_request_id,
            "mark-borrowed",
            &serde_json::json!({}),
            "serialized-borrowed",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(borrowed["status"], "borrowed");
    let status: String =
        sqlx::query_scalar("SELECT status FROM asset_inventory_items WHERE inventory_item_id = $1")
            .bind(item_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(status, "borrowed");

    let returned: serde_json::Value = app
        .post_borrow_request_operation(
            borrow_request_id,
            "return",
            &serde_json::json!({}),
            "serialized-return",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(returned["status"], "returned");
    let inventory_state: (String, f64) = sqlx::query_as(
        "SELECT status, quantity_allocated FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(inventory_state, ("available".to_string(), 0.0));

    let mut transaction_actions: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT action
        FROM inventory_transactions
        WHERE related_resource_id = $1
        "#,
    )
    .bind(borrow_request_id)
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    transaction_actions.sort();
    assert_eq!(
        transaction_actions,
        vec!["allocate", "borrow_out", "release_allocation", "return"]
    );
}

#[tokio::test]
async fn cancelling_an_approved_request_releases_allocated_quantity() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Cancel Owner Lab").await;
    let requester_lab = app.create_laboratory("Cancel Requester Lab").await;
    let item_id = create_quantity_item(&app, owner_lab, "Cancelable Cable", 5.0, true).await;
    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    app.store_user(&requester).await;
    app.store_user(&owner).await;

    requester.login(&app).await;
    let created: serde_json::Value = app
        .post_borrow_request(
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 2.0,
                "purpose": "cancel test"
            }),
            "cancel-create",
        )
        .await
        .json()
        .await
        .unwrap();
    let borrow_request_id: Uuid = created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    owner.login(&app).await;
    app.post_borrow_request_operation(
        borrow_request_id,
        "approve",
        &serde_json::json!({ "comment": "approved" }),
        "cancel-approve",
    )
    .await;
    let allocated: f64 = sqlx::query_scalar(
        "SELECT quantity_allocated FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(allocated, 2.0);

    requester.login(&app).await;
    let cancelled: serde_json::Value = app
        .post_borrow_request_operation(
            borrow_request_id,
            "cancel",
            &serde_json::json!({ "reason": "not needed" }),
            "cancel-key",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    let allocated: f64 = sqlx::query_scalar(
        "SELECT quantity_allocated FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(allocated, 0.0);
}
