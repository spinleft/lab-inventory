use crate::helpers::{TestUser, spawn_app};
use uuid::Uuid;

async fn create_quantity_asset(
    app: &crate::helpers::TestApp,
    laboratory_id: Uuid,
    name: &str,
    unit_id: Uuid,
) -> serde_json::Value {
    app.post_asset(&serde_json::json!({
        "laboratory_id": laboratory_id,
        "asset_kind": "material",
        "tracking_mode": "quantity",
        "name": name,
        "default_unit_id": unit_id,
        "public_notes": "public stock note",
        "internal_notes": "private stock note"
    }))
    .await
    .json()
    .await
    .unwrap()
}

async fn create_quantity_inventory(
    app: &crate::helpers::TestApp,
    asset_id: &serde_json::Value,
    unit_id: Uuid,
    quantity: f64,
    key: &str,
) -> Uuid {
    let item: serde_json::Value = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": asset_id["asset_id"],
                "quantity_on_hand": quantity,
                "unit_id": unit_id,
                "is_cross_lab_borrowable": true
            }),
            key,
        )
        .await
        .json()
        .await
        .unwrap();
    item["inventory_item_id"].as_str().unwrap().parse().unwrap()
}

async fn create_borrow_request(
    app: &crate::helpers::TestApp,
    item_id: Uuid,
    quantity: f64,
    expected_returned_at: Option<&str>,
    key: &str,
) -> Uuid {
    let mut body = serde_json::json!({
        "inventory_item_id": item_id,
        "requested_quantity": quantity,
        "purpose": "shared alert test"
    });
    if let Some(expected_returned_at) = expected_returned_at {
        body["expected_returned_at"] = serde_json::json!(expected_returned_at);
    }
    let created: serde_json::Value = app
        .post_borrow_request(&body, key)
        .await
        .json()
        .await
        .unwrap();
    created["borrow_request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn asset_thresholds_are_permissioned_and_validated() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Threshold Lab").await;
    let other_laboratory_id = app.create_laboratory("Other Threshold Lab").await;
    let meter = app.unit_id("m").await;
    let pcs = app.unit_id("pcs").await;

    let response = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": "Unauth Threshold",
            "default_unit_id": meter,
            "minimum_stock_quantity": 2.0,
            "minimum_stock_unit_id": meter
        }))
        .await;
    assert_eq!(response.status().as_u16(), 401);

    app.test_user.login(&app).await;
    let serialized: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Serialized Threshold",
            "default_unit_id": pcs
        }))
        .await
        .json()
        .await
        .unwrap();
    let response = app
        .patch_asset(
            serialized["asset_id"].as_str().unwrap().parse().unwrap(),
            &serde_json::json!({
                "minimum_stock_quantity": 1.0,
                "minimum_stock_unit_id": pcs
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let asset = create_quantity_asset(&app, laboratory_id, "Threshold Fiber", meter).await;
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let other_asset = create_quantity_asset(&app, other_laboratory_id, "Other Fiber", meter).await;
    let other_asset_id: Uuid = other_asset["asset_id"].as_str().unwrap().parse().unwrap();

    let lab_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&lab_user).await;
    lab_user.login(&app).await;
    let response = app
        .patch_asset(
            asset_id,
            &serde_json::json!({
                "minimum_stock_quantity": 2.0,
                "minimum_stock_unit_id": meter
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let maintainer = TestUser::generate_with_user_type("maintainer", Some(laboratory_id));
    app.store_user(&maintainer).await;
    maintainer.login(&app).await;
    let response = app
        .patch_asset(
            asset_id,
            &serde_json::json!({
                "minimum_stock_quantity": 2.0,
                "minimum_stock_unit_id": meter
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["minimum_stock_quantity"], 2.0);
    assert_eq!(updated["minimum_stock_unit_id"], meter.to_string());

    let response = app
        .patch_asset(
            other_asset_id,
            &serde_json::json!({
                "minimum_stock_quantity": 2.0,
                "minimum_stock_unit_id": meter
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .patch_asset(
            asset_id,
            &serde_json::json!({
                "minimum_stock_quantity": null,
                "minimum_stock_unit_id": null
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let cleared: serde_json::Value = response.json().await.unwrap();
    assert!(cleared["minimum_stock_quantity"].is_null());
    assert!(cleared["minimum_stock_unit_id"].is_null());
}

#[tokio::test]
async fn stock_alerts_follow_available_quantity_and_visibility_rules() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let owner_lab = app.create_laboratory("Stock Alert Owner Lab").await;
    let viewer_lab = app.create_laboratory("Stock Alert Viewer Lab").await;
    let meter = app.unit_id("m").await;

    let asset = create_quantity_asset(&app, owner_lab, "Low Fiber", meter).await;
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let item_id = create_quantity_inventory(&app, &asset, meter, 5.0, "stock-alert-create").await;
    let response = app
        .patch_asset(
            asset_id,
            &serde_json::json!({
                "minimum_stock_quantity": 3.0,
                "minimum_stock_unit_id": meter
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let alerts: serde_json::Value = app.get_stock_alerts().await.json().await.unwrap();
    assert!(alerts.as_array().unwrap().is_empty());

    app.post_inventory_operation(
        item_id,
        "allocate",
        &serde_json::json!({ "quantity": 3.0 }),
        "stock-alert-allocate",
    )
    .await;
    let alerts: serde_json::Value = app.get_stock_alerts().await.json().await.unwrap();
    assert_eq!(alerts.as_array().unwrap().len(), 1);
    assert_eq!(alerts[0]["asset_id"], asset_id.to_string());
    assert_eq!(alerts[0]["quantity_available"], 2.0);
    assert_eq!(alerts[0]["internal_notes"], "private stock note");

    let viewer = TestUser::generate_with_user_type("user", Some(viewer_lab));
    app.store_user(&viewer).await;
    viewer.login(&app).await;
    let alerts: serde_json::Value = app.get_stock_alerts().await.json().await.unwrap();
    assert_eq!(alerts.as_array().unwrap().len(), 1);
    assert_eq!(alerts[0]["asset_id"], asset_id.to_string());
    assert_eq!(alerts[0]["public_notes"], "public stock note");
    assert!(alerts[0]["internal_notes"].is_null());

    app.test_user.login(&app).await;
    app.post_inventory_operation(
        item_id,
        "release-allocation",
        &serde_json::json!({ "quantity": 3.0 }),
        "stock-alert-release",
    )
    .await;
    let alerts: serde_json::Value = app.get_stock_alerts().await.json().await.unwrap();
    assert!(alerts.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn borrow_request_alerts_are_scoped_to_related_laboratories() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let owner_lab = app.create_laboratory("Borrow Alert Owner Lab").await;
    let requester_lab = app.create_laboratory("Borrow Alert Requester Lab").await;
    let unrelated_lab = app.create_laboratory("Borrow Alert Unrelated Lab").await;
    let meter = app.unit_id("m").await;
    let asset = create_quantity_asset(&app, owner_lab, "Borrow Alert Fiber", meter).await;
    let item_id = create_quantity_inventory(&app, &asset, meter, 10.0, "borrow-alert-item").await;

    let requester = TestUser::generate_with_user_type("user", Some(requester_lab));
    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    let unrelated = TestUser::generate_with_user_type("user", Some(unrelated_lab));
    app.store_user(&requester).await;
    app.store_user(&owner).await;
    app.store_user(&unrelated).await;

    requester.login(&app).await;
    let pending_id = create_borrow_request(&app, item_id, 1.0, None, "alert-pending").await;
    let approved_id = create_borrow_request(&app, item_id, 1.0, None, "alert-approved").await;
    let borrowed_id = create_borrow_request(&app, item_id, 1.0, None, "alert-borrowed").await;
    let overdue_id = create_borrow_request(
        &app,
        item_id,
        1.0,
        Some("2000-01-02T00:00:00Z"),
        "alert-overdue",
    )
    .await;

    owner.login(&app).await;
    app.post_borrow_request_operation(
        approved_id,
        "approve",
        &serde_json::json!({ "comment": "approved" }),
        "alert-approve-approved",
    )
    .await;
    app.post_borrow_request_operation(
        borrowed_id,
        "approve",
        &serde_json::json!({ "comment": "approved" }),
        "alert-approve-borrowed",
    )
    .await;
    app.post_borrow_request_operation(
        borrowed_id,
        "mark-borrowed",
        &serde_json::json!({}),
        "alert-mark-borrowed",
    )
    .await;
    app.post_borrow_request_operation(
        overdue_id,
        "approve",
        &serde_json::json!({ "comment": "approved" }),
        "alert-approve-overdue",
    )
    .await;
    app.post_borrow_request_operation(
        overdue_id,
        "mark-borrowed",
        &serde_json::json!({}),
        "alert-mark-overdue",
    )
    .await;

    let owner_alerts: serde_json::Value =
        app.get_borrow_request_alerts().await.json().await.unwrap();
    let owner_alert_ids = owner_alerts
        .as_array()
        .unwrap()
        .iter()
        .map(|alert| {
            (
                alert["borrow_request_id"].as_str().unwrap().to_string(),
                alert["alert_kind"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert!(owner_alert_ids.contains(&(pending_id.to_string(), "pending_approval".to_string())));
    assert!(owner_alert_ids.contains(&(approved_id.to_string(), "pending_borrow_out".to_string())));
    assert!(owner_alert_ids.contains(&(borrowed_id.to_string(), "pending_return".to_string())));
    assert!(owner_alert_ids.contains(&(overdue_id.to_string(), "overdue".to_string())));

    requester.login(&app).await;
    let requester_alerts: serde_json::Value =
        app.get_borrow_request_alerts().await.json().await.unwrap();
    assert_eq!(requester_alerts.as_array().unwrap().len(), 4);

    unrelated.login(&app).await;
    let unrelated_alerts: serde_json::Value =
        app.get_borrow_request_alerts().await.json().await.unwrap();
    assert!(unrelated_alerts.as_array().unwrap().is_empty());
}
