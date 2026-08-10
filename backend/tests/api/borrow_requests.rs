use crate::helpers::{TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn federated_guest_can_request_borrow_and_lab_user_can_approve_it() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Borrow Lab").await;
    let unit_id = app.unit_id("pcs").await;

    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    seed_guest_link(&app, laboratory_id, guest.user_id).await;

    let requester = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&requester).await;
    requester.login(&app).await;

    let asset_id = create_asset(
        &app,
        laboratory_id,
        unit_id,
        "quantity",
        "Borrowable Reagent",
    )
    .await;
    let inventory_item_id = create_inventory_item(&app, asset_id, "B-001").await;

    guest.login(&app).await;
    let response = app
        .post_borrow_request(
            inventory_item_id,
            &serde_json::json!({ "request_note": "Need it for a lab session" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let request: serde_json::Value = response.json().await.unwrap();
    let borrow_request_id = value_uuid(&request["borrow_request_id"]);
    assert_eq!(request["status"], "pending");
    assert_eq!(request["inventory_item_id"], inventory_item_id.to_string());
    assert_eq!(request["requester_user_id"], guest.user_id.to_string());

    requester.login(&app).await;
    let response = app.get_borrow_requests(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let requests: serde_json::Value = response.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(
        requests[0]["borrow_request_id"],
        borrow_request_id.to_string()
    );

    let response = app
        .patch_borrow_request(
            laboratory_id,
            borrow_request_id,
            &serde_json::json!({ "decision": "approved", "decision_note": "Approved for shared use" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let approved: serde_json::Value = response.json().await.unwrap();
    assert_eq!(approved["status"], "approved");
    assert_eq!(
        approved["reviewed_by_user_id"],
        requester.user_id.to_string()
    );

    let response = app.get_inventory_item(inventory_item_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let item: serde_json::Value = response.json().await.unwrap();
    assert_eq!(item["status"], "borrowed");

    let response = app.get_borrow_requests(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let requests: serde_json::Value = response.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(requests[0]["status"], "approved");
}

#[tokio::test]
async fn rejected_borrow_request_remains_available_and_guest_cannot_approve() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Borrow Reject Lab").await;
    let unit_id = app.unit_id("pcs").await;

    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    app.store_user(&guest).await;
    seed_guest_link(&app, laboratory_id, guest.user_id).await;

    let reviewer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&reviewer).await;
    reviewer.login(&app).await;

    let asset_id = create_asset(
        &app,
        laboratory_id,
        unit_id,
        "quantity",
        "Rejectable Reagent",
    )
    .await;
    let inventory_item_id = create_inventory_item(&app, asset_id, "B-002").await;

    guest.login(&app).await;
    let response = app
        .post_borrow_request(
            inventory_item_id,
            &serde_json::json!({ "request_note": "Please reject this one" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let request: serde_json::Value = response.json().await.unwrap();
    let borrow_request_id = value_uuid(&request["borrow_request_id"]);
    reviewer.login(&app).await;

    let response = app
        .patch_borrow_request(
            laboratory_id,
            borrow_request_id,
            &serde_json::json!({ "decision": "rejected" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let rejected: serde_json::Value = response.json().await.unwrap();
    assert_eq!(rejected["status"], "rejected");

    let response = app.get_inventory_item(inventory_item_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let item: serde_json::Value = response.json().await.unwrap();
    assert_eq!(item["status"], "available");

    guest.login(&app).await;
    let response = app
        .patch_borrow_request(
            laboratory_id,
            borrow_request_id,
            &serde_json::json!({ "decision": "approved" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

async fn seed_guest_link(app: &crate::helpers::TestApp, laboratory_id: Uuid, guest_user_id: Uuid) {
    let remote_node_id = Uuid::new_v4();
    let remote_laboratory_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO federation_remote_nodes (
            remote_node_id,
            base_url,
            display_name,
            shared_secret,
            shared_secret_hash,
            status,
            key_version
        )
        VALUES ($1, $2, $3, $4, $5, 'active', 1)
        "#,
    )
    .bind(remote_node_id)
    .bind(format!("https://{}.example.com", remote_node_id))
    .bind("Remote Lab")
    .bind("shared-secret")
    .bind("shared-secret-hash")
    .execute(&app.db_pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO federation_guest_links (
            link_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_user_id,
            remote_username,
            remote_user_type,
            local_guest_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(remote_node_id)
    .bind(remote_laboratory_id)
    .bind(Uuid::new_v4())
    .bind("remote-user")
    .bind("user")
    .bind(guest_user_id)
    .execute(&app.db_pool)
    .await
    .unwrap();
}

async fn create_asset(
    app: &crate::helpers::TestApp,
    laboratory_id: Uuid,
    unit_id: Uuid,
    tracking_mode: &str,
    name: &str,
) -> Uuid {
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": tracking_mode,
                "name": name,
                "inventory_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    value_uuid(&asset["asset_id"])
}

async fn create_inventory_item(
    app: &crate::helpers::TestApp,
    asset_id: Uuid,
    batch_number: &str,
) -> Uuid {
    let response = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "batch_number": batch_number,
                "quantity_on_hand": 1,
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    value_uuid(&items[0]["inventory_item_id"])
}

fn value_uuid(value: &serde_json::Value) -> Uuid {
    Uuid::parse_str(value.as_str().unwrap()).unwrap()
}
