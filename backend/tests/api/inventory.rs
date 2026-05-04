use crate::helpers::{TestUser, spawn_app};
use reqwest::header::CONTENT_TYPE;

#[tokio::test]
async fn inventory_writes_require_an_authorized_laboratory_member() {
    let app = spawn_app().await;
    let own_lab = app.create_laboratory("Inventory Own Lab").await;
    let other_lab = app.create_laboratory("Inventory Other Lab").await;
    let pcs = app.unit_id("pcs").await;

    let response = app
        .post_asset(&serde_json::json!({
            "laboratory_id": own_lab,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Oscilloscope",
            "default_unit_id": pcs
        }))
        .await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_group("guest", Some(own_lab));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .post_location(&serde_json::json!({
            "name": "Cabinet A"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let lab_user = TestUser::generate_with_group("user", Some(own_lab));
    app.store_user(&lab_user).await;
    lab_user.login(&app).await;
    let response = app
        .post_asset_category(&serde_json::json!({
            "laboratory_id": other_lab,
            "name": "Forbidden Category"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .post_asset_category(&serde_json::json!({
            "name": "Optics"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);

    app.test_user.login(&app).await;
    let response = app
        .post_location(&serde_json::json!({
            "laboratory_id": other_lab,
            "name": "Shared Shelf"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app.get_units().await;
    assert_eq!(response.status().as_u16(), 200);
    let units: serde_json::Value = response.json().await.unwrap();
    assert!(
        units
            .as_array()
            .unwrap()
            .iter()
            .any(|unit| unit["code"] == "pcs")
    );
}

#[tokio::test]
async fn serialized_and_quantity_inventory_validation_follows_tracking_mode() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let lab_id = app.create_laboratory("Tracking Lab").await;
    let pcs = app.unit_id("pcs").await;
    let meter = app.unit_id("m").await;

    let response = app
        .post_asset(&serde_json::json!({
            "laboratory_id": lab_id,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "AWG",
            "model": "M8195A",
            "default_unit_id": pcs
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let serialized_asset: serde_json::Value = response.json().await.unwrap();
    let serialized_asset_id = serialized_asset["asset_id"].as_str().unwrap();

    let response = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": serialized_asset_id,
                "quantity_on_hand": 1
            }),
            "missing-serial",
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let serialized_body = serde_json::json!({
        "asset_id": serialized_asset_id,
        "serial_number": "SN-001",
        "internal_notes": "calibration due",
        "public_notes": "borrowable generator"
    });
    let response = app
        .post_inventory_item(&serialized_body, "serialized-create")
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let item: serde_json::Value = response.json().await.unwrap();
    assert_eq!(item["serial_number"], "SN-001");
    assert_eq!(item["quantity_on_hand"], 1.0);

    let response = app
        .post_inventory_item(&serialized_body, "serialized-duplicate")
        .await;
    assert_eq!(response.status().as_u16(), 409);

    let response = app
        .post_asset(&serde_json::json!({
            "laboratory_id": lab_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": "Optical Fiber",
            "default_unit_id": meter
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let quantity_asset: serde_json::Value = response.json().await.unwrap();
    let quantity_asset_id = quantity_asset["asset_id"].as_str().unwrap();

    let response = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": quantity_asset_id,
                "serial_number": "Q-001",
                "quantity_on_hand": 5.5,
                "unit_id": meter
            }),
            "quantity-with-serial",
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": quantity_asset_id,
                "quantity_on_hand": 5.5,
                "unit_id": meter
            }),
            "quantity-create",
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let quantity_item: serde_json::Value = response.json().await.unwrap();
    assert_eq!(quantity_item["quantity_available"], 5.5);
}

#[tokio::test]
async fn inventory_operations_update_quantities_and_record_transactions() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let lab_id = app.create_laboratory("Operations Lab").await;
    let meter = app.unit_id("m").await;

    let location_a: serde_json::Value = app
        .post_location(&serde_json::json!({
            "laboratory_id": lab_id,
            "name": "Shelf A"
        }))
        .await
        .json()
        .await
        .unwrap();
    let location_b: serde_json::Value = app
        .post_location(&serde_json::json!({
            "laboratory_id": lab_id,
            "name": "Shelf B"
        }))
        .await
        .json()
        .await
        .unwrap();

    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": lab_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": "Cable",
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
                "quantity_on_hand": 5.5,
                "unit_id": meter,
                "location_id": location_a["location_id"]
            }),
            "ops-create",
        )
        .await
        .json()
        .await
        .unwrap();
    let inventory_item_id: uuid::Uuid =
        item["inventory_item_id"].as_str().unwrap().parse().unwrap();

    let response = app
        .post_inventory_operation(
            inventory_item_id,
            "adjust",
            &serde_json::json!({ "quantity_delta": 2.0 }),
            "ops-adjust",
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let adjusted: serde_json::Value = response.json().await.unwrap();
    assert_eq!(adjusted["quantity_on_hand"], 7.5);

    let response = app
        .post_inventory_operation(
            inventory_item_id,
            "adjust",
            &serde_json::json!({ "quantity_delta": 2.0 }),
            "ops-adjust",
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let repeated_adjust: serde_json::Value = response.json().await.unwrap();
    assert_eq!(repeated_adjust["quantity_on_hand"], 7.5);

    let allocated: serde_json::Value = app
        .post_inventory_operation(
            inventory_item_id,
            "allocate",
            &serde_json::json!({ "quantity": 2.0 }),
            "ops-allocate",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(allocated["quantity_allocated"], 2.0);
    assert_eq!(allocated["quantity_available"], 5.5);

    let released: serde_json::Value = app
        .post_inventory_operation(
            inventory_item_id,
            "release-allocation",
            &serde_json::json!({ "quantity": 1.0 }),
            "ops-release",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(released["quantity_allocated"], 1.0);

    let moved: serde_json::Value = app
        .post_inventory_operation(
            inventory_item_id,
            "move",
            &serde_json::json!({ "location_id": location_b["location_id"] }),
            "ops-move",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(moved["location_id"], location_b["location_id"]);

    let counted: serde_json::Value = app
        .post_inventory_operation(
            inventory_item_id,
            "stocktake",
            &serde_json::json!({ "quantity_on_hand": 4.0 }),
            "ops-stocktake",
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(counted["quantity_on_hand"], 4.0);
    assert_eq!(counted["quantity_available"], 3.0);

    let transaction_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_transactions WHERE inventory_item_id = $1",
    )
    .bind(inventory_item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(transaction_count, 6);
}

#[tokio::test]
async fn idempotency_is_scoped_to_users_and_replays_saved_response_headers() {
    let app = spawn_app().await;
    let lab_id = app.create_laboratory("Idempotency Lab").await;
    let meter = app.unit_id("m").await;
    let lab_user = TestUser::generate_with_group("user", Some(lab_id));
    app.store_user(&lab_user).await;

    app.test_user.login(&app).await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": lab_id,
            "asset_kind": "material",
            "tracking_mode": "quantity",
            "name": "Tubing",
            "default_unit_id": meter
        }))
        .await
        .json()
        .await
        .unwrap();
    let body = serde_json::json!({
        "asset_id": asset["asset_id"],
        "quantity_on_hand": 1.0,
        "unit_id": meter
    });

    let response = app.post_inventory_item(&body, "same-key").await;
    assert_eq!(response.status().as_u16(), 201);
    assert!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let first: serde_json::Value = response.json().await.unwrap();
    let response = app.post_inventory_item(&body, "same-key").await;
    assert_eq!(response.status().as_u16(), 201);
    assert!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let second: serde_json::Value = response.json().await.unwrap();
    assert_eq!(first["inventory_item_id"], second["inventory_item_id"]);

    lab_user.login(&app).await;
    let response = app.post_inventory_item(&body, "same-key").await;
    assert_eq!(response.status().as_u16(), 201);
    let third: serde_json::Value = response.json().await.unwrap();
    assert_ne!(first["inventory_item_id"], third["inventory_item_id"]);
}

#[tokio::test]
async fn other_laboratory_members_only_see_public_inventory_fields() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let owner_lab = app.create_laboratory("Owner Lab").await;
    let viewer_lab = app.create_laboratory("Viewer Lab").await;
    let pcs = app.unit_id("pcs").await;
    let viewer = TestUser::generate_with_group("user", Some(viewer_lab));
    app.store_user(&viewer).await;

    let location: serde_json::Value = app
        .post_location(&serde_json::json!({
            "laboratory_id": owner_lab,
            "name": "Private Cabinet"
        }))
        .await
        .json()
        .await
        .unwrap();
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": owner_lab,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Spectrum Analyzer",
            "default_unit_id": pcs,
            "public_notes": "available by request",
            "internal_notes": "expensive probe set"
        }))
        .await
        .json()
        .await
        .unwrap();
    app.post_inventory_item(
        &serde_json::json!({
            "asset_id": asset["asset_id"],
            "serial_number": "SA-SECRET",
            "location_id": location["location_id"],
            "is_cross_lab_borrowable": true,
            "public_notes": "can borrow with approval",
            "internal_notes": "do not disclose serial"
        }),
        "visibility-create",
    )
    .await;

    viewer.login(&app).await;
    let assets: serde_json::Value = app.get_assets().await.json().await.unwrap();
    let analyzer = assets["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["name"] == "Spectrum Analyzer")
        .unwrap();
    assert_eq!(analyzer["public_notes"], "available by request");
    assert!(analyzer["internal_notes"].is_null());

    let items: serde_json::Value = app.get_inventory_items().await.json().await.unwrap();
    let item = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["asset_name"] == "Spectrum Analyzer")
        .unwrap();
    assert_eq!(item["public_notes"], "can borrow with approval");
    assert_eq!(item["is_cross_lab_borrowable"], true);
    assert!(item["serial_number"].is_null());
    assert!(item["location_id"].is_null());
    assert!(item["internal_notes"].is_null());
}
