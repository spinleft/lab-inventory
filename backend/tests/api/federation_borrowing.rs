use crate::helpers::{TestApp, TestUser, pair_laboratories, sign_federation_request, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn remote_user_can_request_borrow_and_read_status_over_federation() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Federation Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Federation Remote").await;
    let inventory_item_id = seed_borrowable_item(&remote, remote_laboratory_id, "F-001").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;

    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({ "request_note": "Needed for a joint experiment" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let request: serde_json::Value = response.json().await.unwrap();
    assert_eq!(request["status"], "pending");
    assert_eq!(request["inventory_item_id"], inventory_item_id.to_string());
    assert_eq!(request["request_note"], "Needed for a joint experiment");
    // The lending laboratory's staff and its internal identifiers for this
    // caller are not part of what a partner is told.
    assert!(request.get("reviewed_by_username").is_none());
    assert!(request.get("reviewed_by_user_id").is_none());
    assert!(request.get("requester_username").is_none());
    assert!(request.get("requester_guest_link_id").is_none());
    assert!(request.get("inventory_item_title").is_none());

    let response = local
        .get_federation_borrow_requests(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let requests: serde_json::Value = response.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(requests[0]["status"], "pending");

    // The lending laboratory sees it in its own queue, filed against the shadow
    // guest the remote user acts as.
    let remote_reviewer = TestUser::generate_with_user_type("user", Some(remote_laboratory_id));
    remote.store_user(&remote_reviewer).await;
    remote_reviewer.login(&remote).await;
    let response = remote.get_borrow_requests(remote_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let queue: serde_json::Value = response.json().await.unwrap();
    assert_eq!(queue.as_array().unwrap().len(), 1);
    assert_eq!(queue[0]["requester_user_type"], "guest");
    assert!(!queue[0]["requester_guest_link_id"].is_null());
}

#[tokio::test]
async fn remote_requester_can_cancel_before_approval_and_ask_again() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Cancel Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Cancel Remote").await;
    let inventory_item_id = seed_borrowable_item(&remote, remote_laboratory_id, "F-002").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;

    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let request: serde_json::Value = response.json().await.unwrap();
    let borrow_request_id = value_uuid(&request["borrow_request_id"]);

    // A second request while the first is pending is a conflict, not a 500.
    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = local
        .post_federation_borrow_request_cancel(
            remote_node_id,
            remote_laboratory_id,
            borrow_request_id,
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let cancelled: serde_json::Value = response.json().await.unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    // Nobody decided on it, so no reviewer was recorded.
    assert!(cancelled["reviewed_at"].is_null());

    let status: String =
        sqlx::query_scalar("SELECT status FROM asset_inventory_items WHERE inventory_item_id = $1")
            .bind(inventory_item_id)
            .fetch_one(&remote.db_pool)
            .await
            .unwrap();
    assert_eq!(status, "available");

    // Cancelling drops the request out of the partial unique index, so the item
    // can be asked for again.
    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
}

#[tokio::test]
async fn remote_requester_cannot_cancel_after_approval() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Late Cancel Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Late Cancel Remote").await;
    let inventory_item_id = seed_borrowable_item(&remote, remote_laboratory_id, "F-003").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;
    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let request: serde_json::Value = response.json().await.unwrap();
    let borrow_request_id = value_uuid(&request["borrow_request_id"]);

    let remote_reviewer = TestUser::generate_with_user_type("user", Some(remote_laboratory_id));
    remote.store_user(&remote_reviewer).await;
    remote_reviewer.login(&remote).await;
    let response = remote
        .patch_borrow_request(
            remote_laboratory_id,
            borrow_request_id,
            &serde_json::json!({ "decision": "approved" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    local_user.login(&local).await;
    let response = local
        .post_federation_borrow_request_cancel(
            remote_node_id,
            remote_laboratory_id,
            borrow_request_id,
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);
}

#[tokio::test]
async fn a_federated_reader_sees_only_their_own_requests() {
    let lender = spawn_app().await;
    let first = spawn_app().await;
    let second = spawn_app().await;
    let lender_laboratory_id = lender.create_laboratory("Borrow Isolation Lender").await;
    let first_laboratory_id = first.create_laboratory("Borrow Isolation First").await;
    let second_laboratory_id = second.create_laboratory("Borrow Isolation Second").await;
    let first_item = seed_borrowable_item(&lender, lender_laboratory_id, "F-004").await;
    let second_item = seed_borrowable_item(&lender, lender_laboratory_id, "F-005").await;

    let first_node_id =
        pair_laboratories(&first, first_laboratory_id, &lender, lender_laboratory_id).await;
    let second_node_id =
        pair_laboratories(&second, second_laboratory_id, &lender, lender_laboratory_id).await;

    let first_user = TestUser::generate_with_user_type("user", Some(first_laboratory_id));
    first.store_user(&first_user).await;
    first_user.login(&first).await;
    let response = first
        .post_federation_borrow_request(
            first_node_id,
            lender_laboratory_id,
            first_item,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let second_user = TestUser::generate_with_user_type("user", Some(second_laboratory_id));
    second.store_user(&second_user).await;
    second_user.login(&second).await;
    let response = second
        .post_federation_borrow_request(
            second_node_id,
            lender_laboratory_id,
            second_item,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    // Two requests exist at the lender, and each reader sees exactly their own.
    first_user.login(&first).await;
    let response = first
        .get_federation_borrow_requests(first_node_id, lender_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let requests: serde_json::Value = response.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(requests[0]["inventory_item_id"], first_item.to_string());

    let response = second
        .get_federation_borrow_requests(second_node_id, lender_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let requests: serde_json::Value = response.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(requests[0]["inventory_item_id"], second_item.to_string());
}

#[tokio::test]
async fn inbound_write_requires_a_valid_signature() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Unsigned Borrow Lab").await;
    let response = app
        .api_client
        .post(format!(
            "{}/api/v1/federation/inbound/laboratories/{laboratory_id}/inventory-items/{}/borrow-requests",
            app.address,
            Uuid::new_v4()
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(response.status().as_u16(), 401);
}

/// The one test that proves the inbound signature actually covers the request
/// body. It would pass just as well against a verifier that hashed an empty
/// slice, were it not for the fact that the signature is made over different
/// bytes than the ones sent.
#[tokio::test]
async fn inbound_write_rejects_a_tampered_body() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Tamper Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Tamper Remote").await;
    let inventory_item_id = seed_borrowable_item(&remote, remote_laboratory_id, "F-006").await;
    pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;

    let path = format!(
        "/api/v1/federation/inbound/laboratories/{remote_laboratory_id}/inventory-items/{inventory_item_id}/borrow-requests"
    );
    let signed_body = serde_json::to_vec(&serde_json::json!({ "request_note": "honest" })).unwrap();
    let sent_body = serde_json::to_vec(&serde_json::json!({ "request_note": "swapped" })).unwrap();
    let headers = sign_federation_request(
        &local,
        &remote,
        "POST",
        &path,
        &signed_body,
        local_laboratory_id,
        local_user.user_id,
        "user",
    )
    .await;

    let mut request = remote
        .api_client
        .post(format!("{}{path}", remote.address))
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request
        .body(sent_body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(response.status().as_u16(), 401);

    // The same headers over the body they were made for are accepted, which is
    // what rules out the request having been rejected for some other reason.
    let mut request = remote
        .api_client
        .post(format!("{}{path}", remote.address))
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request
        .body(signed_body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(response.status().as_u16(), 201);
}

#[tokio::test]
async fn inbound_post_refuses_a_path_that_is_only_readable() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Read Path Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Read Path Remote").await;
    pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;

    let path = format!("/api/v1/federation/inbound/laboratories/{remote_laboratory_id}/assets");
    let body = serde_json::to_vec(&serde_json::json!({})).unwrap();
    let headers = sign_federation_request(
        &local,
        &remote,
        "POST",
        &path,
        &body,
        local_laboratory_id,
        local_user.user_id,
        "user",
    )
    .await;

    let mut request = remote
        .api_client
        .post(format!("{}{path}", remote.address))
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn guests_and_server_admins_cannot_use_the_federation_write_proxy() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Borrow Proxy Role Local").await;
    let remote_laboratory_id = remote.create_laboratory("Borrow Proxy Role Remote").await;
    let inventory_item_id = seed_borrowable_item(&remote, remote_laboratory_id, "F-007").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    local.test_user.login(&local).await;
    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let guest = TestUser::generate_with_user_type("guest", Some(local_laboratory_id));
    local.store_user(&guest).await;
    guest.login(&local).await;
    let response = local
        .post_federation_borrow_request(
            remote_node_id,
            remote_laboratory_id,
            inventory_item_id,
            &serde_json::json!({}),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

async fn seed_borrowable_item(app: &TestApp, laboratory_id: Uuid, batch_number: &str) -> Uuid {
    app.test_user.login(app).await;
    let unit_id = app.unit_id("pcs").await;
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": format!("Borrowable {batch_number}"),
                "inventory_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = value_uuid(&asset["asset_id"]);

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
