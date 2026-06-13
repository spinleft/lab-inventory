use crate::helpers::{TestUser, spawn_app};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

async fn create_quantity_item(
    app: &crate::helpers::TestApp,
    laboratory_id: Uuid,
    name: &str,
    quantity: f64,
) -> Uuid {
    app.test_user.login(app).await;
    let meter = app.unit_id("m").await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": name,
            "default_unit_id": meter,
            "internal_notes": "secret asset notes"
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
                "is_cross_lab_borrowable": true,
                "batch_number": "PRIVATE-BATCH",
                "internal_notes": "secret stock notes"
            }),
            &format!("{name}-inventory"),
        )
        .await
        .json()
        .await
        .unwrap();
    item["inventory_item_id"].as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn migrations_seed_only_admin_and_user_roles() {
    let app = spawn_app().await;

    let user_types: Vec<String> = sqlx::query_scalar("SELECT name FROM user_types ORDER BY name")
        .fetch_all(&app.db_pool)
        .await
        .unwrap();

    assert_eq!(user_types, vec!["admin", "user"]);
}

#[tokio::test]
async fn admin_can_manage_remote_laboratories_without_secret_echo() {
    let app = spawn_app().await;
    let remote_id = Uuid::new_v4();

    app.test_user.login(&app).await;
    let created: serde_json::Value = app
        .post_remote_laboratory(&serde_json::json!({
            "remote_laboratory_id": remote_id,
            "name": "远端材料实验室",
            "api_base_url": "http://127.0.0.1:18080/api/v1",
            "is_enabled": true,
            "key_id": "demo-key",
            "shared_secret": "demo-secret"
        }))
        .await
        .json()
        .await
        .unwrap();

    assert_eq!(created["remote_laboratory_id"], remote_id.to_string());
    assert!(created.get("shared_secret").is_none());

    let listed: serde_json::Value = app.get_remote_laboratories().await.json().await.unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("shared_secret").is_none());

    let user = TestUser::generate_with_user_type("user", Some(app.local_laboratory_id));
    app.store_user(&user).await;
    user.login(&app).await;
    let response = app
        .post_remote_laboratory(&serde_json::json!({
            "remote_laboratory_id": Uuid::new_v4(),
            "name": "Forbidden Lab",
            "api_base_url": "http://127.0.0.1:18081/api/v1",
            "is_enabled": true,
            "key_id": "forbidden-key",
            "shared_secret": "forbidden-secret"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn federation_inventory_requires_signature_and_returns_public_fields_only() {
    let app = spawn_app().await;
    let caller_lab_id = Uuid::new_v4();
    let secret = "shared-demo-secret";
    app.insert_remote_laboratory(
        caller_lab_id,
        "Caller Lab",
        &app.address,
        "caller-key",
        secret,
    )
    .await;
    create_quantity_item(&app, app.local_laboratory_id, "Public Fiber", 5.0).await;
    app.post_logout().await;

    let unsigned = app.get_api_path("/federation/inventory-items").await;
    assert_eq!(unsigned.status().as_u16(), 401);

    let path = "/api/v1/federation/inventory-items";
    let response = app
        .api_client
        .get(format!("{}{}", app.address, path))
        .headers(sign_headers(
            "GET",
            path,
            "",
            caller_lab_id,
            "caller-key",
            secret,
        ))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    assert_eq!(status.as_u16(), 200, "{text}");
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    let item = &body["items"][0];
    assert_eq!(item["asset_name"], "Public Fiber");
    assert_eq!(item["quantity_available"], 5.0);
    assert!(item.get("serial_number").is_none());
    assert!(item.get("batch_number").is_none());
    assert!(item.get("internal_notes").is_none());
}

#[tokio::test]
async fn remote_borrow_request_creates_matching_rows_on_both_nodes() {
    let requester = spawn_app().await;
    let owner = spawn_app().await;
    let shared_secret = "borrow-shared-secret";
    let item_id =
        create_quantity_item(&owner, owner.local_laboratory_id, "Shared Solvent", 8.0).await;

    requester
        .insert_remote_laboratory(
            owner.local_laboratory_id,
            "Owner Lab",
            &owner.address,
            "owner-key",
            shared_secret,
        )
        .await;
    owner
        .insert_remote_laboratory(
            requester.local_laboratory_id,
            "Requester Lab",
            &requester.address,
            "owner-key",
            shared_secret,
        )
        .await;

    let requester_user =
        TestUser::generate_with_user_type("user", Some(requester.local_laboratory_id));
    requester.store_user(&requester_user).await;
    requester_user.login(&requester).await;

    let created: serde_json::Value = requester
        .post_remote_borrow_request(
            owner.local_laboratory_id,
            &serde_json::json!({
                "inventory_item_id": item_id,
                "requested_quantity": 2.0,
                "purpose": "federated demo"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let correlation_id: Uuid = created["correlation_id"].as_str().unwrap().parse().unwrap();

    let requester_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM borrow_requests WHERE correlation_id = $1")
            .bind(correlation_id)
            .fetch_one(&requester.db_pool)
            .await
            .unwrap();
    let owner_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM borrow_requests WHERE correlation_id = $1")
            .bind(correlation_id)
            .fetch_one(&owner.db_pool)
            .await
            .unwrap();

    assert_eq!(requester_rows, 1);
    assert_eq!(owner_rows, 1);
}

fn sign_headers(
    method: &str,
    path_and_query: &str,
    body: &str,
    lab_id: Uuid,
    key_id: &str,
    secret: &str,
) -> reqwest::header::HeaderMap {
    let timestamp = Utc::now().to_rfc3339();
    let nonce = Uuid::new_v4().to_string();
    let body_hash = hex::encode(Sha256::digest(body.as_bytes()));
    let signing_string = format!("{method}\n{path_and_query}\n{timestamp}\n{nonce}\n{body_hash}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signing_string.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("X-Lab-Id", lab_id.to_string().parse().unwrap());
    headers.insert("X-Key-Id", key_id.parse().unwrap());
    headers.insert("X-Timestamp", timestamp.parse().unwrap());
    headers.insert("X-Nonce", nonce.parse().unwrap());
    headers.insert("X-Signature", signature.parse().unwrap());
    headers
}
