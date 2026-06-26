use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_list_get_assets_with_inventory_parameters_and_audit() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Asset Api Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let parameter = create_text_parameter(&app, laboratory_id, "color").await;
    let category = create_category_with_required_parameter(
        &app,
        laboratory_id,
        "Instruments",
        "instruments",
        parameter_id(&parameter),
    )
    .await;

    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "category_id": category_id(&category),
                "tracking_mode": "quantity",
                "name": "Oscilloscope Probe",
                "model": "P2200",
                "manufacturer": "Tektronix",
                "default_unit_id": unit_id,
                "public_notes": "Shared probe",
                "inventory_items": [
                    {
                        "batch_number": "BATCH-001",
                        "quantity_on_hand": 5,
                        "quantity_allocated": 1,
                        "quantity_unit_id": unit_id,
                        "status": "available"
                    }
                ],
                "parameters": [
                    {
                        "parameter_type_id": parameter_id(&parameter),
                        "value": { "text": "blue" }
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = asset_id(&asset);
    assert_eq!(asset["laboratory_id"], laboratory_id.to_string());
    assert_eq!(asset["tracking_mode"], "quantity");
    assert_eq!(asset["inventory_summary"]["item_count"], 1);
    assert_eq!(asset["inventory_items"].as_array().unwrap().len(), 1);
    assert_eq!(asset["parameters"].as_array().unwrap().len(), 1);
    assert_eq!(asset["parameters"][0]["value"]["text"], "blue");

    let audit_details = latest_audit_details(&app, asset_id, "create").await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(audit_details["rollback"]["resource_type"], "asset");

    let response = app
        .get_assets_with_query(
            laboratory_id,
            &format!(
                "include=parameters&keyword=Oscilloscope&category_id={}",
                category_id(&category)
            ),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["asset_id"], asset_id.to_string());
    assert_eq!(body["items"][0]["parameters"][0]["value"]["text"], "blue");

    let response = app.get_asset(asset_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("parameters").is_none());
    assert_eq!(body["inventory_items"].as_array().unwrap().len(), 1);

    let response = app
        .get_asset_with_query(asset_id, "include=parameters")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["parameters"][0]["value"]["text"], "blue");
}

#[tokio::test]
async fn create_asset_rejects_invalid_inventory_required_parameters_and_duplicates() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Asset Validation Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let other_unit_id = app.unit_id("cm").await;
    app.test_user.login(&app).await;

    let parameter = create_text_parameter(&app, laboratory_id, "required_text").await;
    let category = create_category_with_required_parameter(
        &app,
        laboratory_id,
        "Required",
        "required",
        parameter_id(&parameter),
    )
    .await;

    let missing_required = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "category_id": category_id(&category),
                "tracking_mode": "quantity",
                "name": "Missing Required",
                "default_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(missing_required.status().as_u16(), 400);

    let serialized_with_quantity = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "serialized",
                "name": "Serialized With Quantity",
                "default_unit_id": unit_id,
                "inventory_items": [
                    { "serial_number": "SN-1", "quantity_on_hand": 1 }
                ]
            }),
        )
        .await;
    assert_eq!(serialized_with_quantity.status().as_u16(), 400);

    let quantity_with_serial = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Quantity With Serial",
                "default_unit_id": unit_id,
                "inventory_items": [
                    { "serial_number": "SN-2", "quantity_on_hand": 1 }
                ]
            }),
        )
        .await;
    assert_eq!(quantity_with_serial.status().as_u16(), 400);

    let inventory_unit_mismatch = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Inventory Unit Mismatch",
                "default_unit_id": unit_id,
                "inventory_items": [
                    {
                        "batch_number": "UNIT-MISMATCH",
                        "quantity_on_hand": 1,
                        "quantity_unit_id": other_unit_id
                    }
                ]
            }),
        )
        .await;
    assert_eq!(inventory_unit_mismatch.status().as_u16(), 400);

    let body = serde_json::json!({
        "tracking_mode": "quantity",
        "name": "Duplicated Asset",
        "model": "DUP",
        "default_unit_id": unit_id
    });
    let response = app.post_asset(laboratory_id, &body).await;
    assert_eq!(response.status().as_u16(), 201);
    let response = app.post_asset(laboratory_id, &body).await;
    assert_eq!(response.status().as_u16(), 409);
}

#[tokio::test]
async fn update_asset_applies_partial_changes_parameters_and_tracking_mode_rules() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Asset Update Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let parameter = create_text_parameter(&app, laboratory_id, "label").await;
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Configurable Asset",
                "default_unit_id": unit_id,
                "parameters": [
                    {
                        "parameter_type_id": parameter_id(&parameter),
                        "value": { "text": "old" }
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let configurable_asset_id = asset_id(&asset);

    let response = app
        .patch_asset(
            configurable_asset_id,
            &serde_json::json!({
                "tracking_mode": "serialized",
                "name": "Configurable Asset Updated",
                "parameters": [
                    {
                        "parameter_type_id": parameter_id(&parameter),
                        "value": null
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["tracking_mode"], "serialized");
    assert_eq!(updated["name"], "Configurable Asset Updated");
    assert!(updated["parameters"].as_array().unwrap().is_empty());

    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Stocked Asset",
                "default_unit_id": unit_id,
                "inventory_items": [
                    { "batch_number": "STOCK", "quantity_on_hand": 1 }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let stocked: serde_json::Value = response.json().await.unwrap();
    let response = app
        .patch_asset(
            asset_id(&stocked),
            &serde_json::json!({ "tracking_mode": "serialized" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn update_asset_default_unit_converts_inventory_quantities() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Asset Unit Conversion Lab").await;
    let centimeter_unit_id = app.unit_id("cm").await;
    let meter_unit_id = app.unit_id("m").await;
    app.test_user.login(&app).await;

    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Cable",
                "default_unit_id": centimeter_unit_id,
                "inventory_items": [
                    {
                        "batch_number": "CUT-1",
                        "quantity_on_hand": 250,
                        "quantity_allocated": 50
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let inventory_item_id = Uuid::parse_str(
        asset["inventory_items"][0]["inventory_item_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let response = app
        .patch_asset(
            asset_id(&asset),
            &serde_json::json!({ "default_unit_id": meter_unit_id }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["default_unit_id"], meter_unit_id.to_string());
    assert_eq!(
        updated["inventory_items"][0]["quantity_unit_id"],
        meter_unit_id.to_string()
    );
    assert_eq!(updated["inventory_items"][0]["quantity_on_hand"], 2.5);
    assert_eq!(updated["inventory_items"][0]["quantity_allocated"], 0.5);
    assert_eq!(updated["inventory_summary"]["quantity_on_hand"], 2.5);
    assert_eq!(updated["inventory_summary"]["quantity_allocated"], 0.5);

    let stored: (f64, f64, Uuid) = sqlx::query_as(
        r#"
        SELECT
            quantity_on_hand::double precision,
            quantity_allocated::double precision,
            quantity_unit_id
        FROM asset_inventory_items
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(inventory_item_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(stored, (2.5, 0.5, meter_unit_id));
}

#[tokio::test]
async fn delete_asset_cascades_inventory_parameter_values_and_attachments() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Asset Delete Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let parameter = create_text_parameter(&app, laboratory_id, "delete_label").await;
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Delete Me",
                "default_unit_id": unit_id,
                "inventory_items": [
                    { "batch_number": "DEL", "quantity_on_hand": 1 }
                ],
                "parameters": [
                    {
                        "parameter_type_id": parameter_id(&parameter),
                        "value": { "text": "temporary" }
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = asset_id(&asset);
    let inventory_item_id = Uuid::parse_str(
        asset["inventory_items"][0]["inventory_item_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let asset_attachment_id = insert_attachment(&app, laboratory_id, "asset", asset_id).await;
    let inventory_attachment_id =
        insert_attachment(&app, laboratory_id, "inventory_item", inventory_item_id).await;

    let response = app.delete_asset(asset_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE asset_id = $1")
        .bind(asset_id)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(asset_count, 0);

    let inventory_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asset_inventory_items WHERE asset_id = $1")
            .bind(asset_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(inventory_count, 0);

    let parameter_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asset_parameter_values WHERE asset_id = $1")
            .bind(asset_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(parameter_count, 0);

    let attachment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE attachment_id = ANY($1)")
            .bind(vec![asset_attachment_id, inventory_attachment_id])
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(attachment_count, 0);
}

#[tokio::test]
async fn asset_permissions_follow_laboratory_scope() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Asset Permission Own").await;
    let other_laboratory_id = app.create_laboratory("Asset Permission Other").await;
    let unit_id = app.unit_id("pcs").await;

    app.test_user.login(&app).await;
    let response = app
        .post_asset(
            other_laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Other Asset",
                "default_unit_id": unit_id,
                "internal_notes": "Other lab internal notes"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let other_asset: serde_json::Value = response.json().await.unwrap();

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app
        .post_asset(
            own_laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Own Asset",
                "default_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();

    let response = app.get_assets(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app.get_assets(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["items"][0]["asset_id"],
        asset_id(&other_asset).to_string()
    );
    assert!(body["items"][0]["internal_notes"].is_null());

    let response = app.get_asset(asset_id(&other_asset)).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["internal_notes"].is_null());

    let response = app
        .post_asset(
            other_laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Other Asset",
                "default_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app.get_asset(asset_id(&asset)).await;
    assert_eq!(response.status().as_u16(), 200);

    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .patch_asset(
            asset_id(&asset),
            &serde_json::json!({ "name": "Guest Update" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

async fn create_text_parameter(
    app: &TestApp,
    laboratory_id: Uuid,
    code: &str,
) -> serde_json::Value {
    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": code,
                "name": format!("Parameter {code}"),
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

async fn create_category_with_required_parameter(
    app: &TestApp,
    laboratory_id: Uuid,
    name: &str,
    code: &str,
    parameter_type_id: Uuid,
) -> serde_json::Value {
    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": name,
                "code": code,
                "parameter_assignments": [
                    {
                        "parameter_type_id": parameter_type_id,
                        "is_required": true
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

async fn insert_attachment(
    app: &TestApp,
    laboratory_id: Uuid,
    resource_type: &str,
    resource_id: Uuid,
) -> Uuid {
    let (asset_id, inventory_item_id) = match resource_type {
        "asset" => (Some(resource_id), None),
        "inventory_item" => (None, Some(resource_id)),
        _ => panic!("unsupported resource type"),
    };
    let storage_key = format!("labs/{laboratory_id}/objects/{}/asset.txt", Uuid::new_v4());
    sqlx::query_scalar(
        r#"
        INSERT INTO attachments (
            attachment_id,
            laboratory_id,
            asset_id,
            inventory_item_id,
            display_name,
            original_file_name,
            file_size_bytes,
            sha256_hex,
            storage_backend,
            storage_key
        )
        VALUES ($1, $2, $3, $4, 'asset.txt', 'asset.txt', 1, $5, 'local', $6)
        RETURNING attachment_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(asset_id)
    .bind(inventory_item_id)
    .bind("a".repeat(64))
    .bind(storage_key)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

fn asset_id(asset: &serde_json::Value) -> Uuid {
    Uuid::parse_str(asset["asset_id"].as_str().unwrap()).unwrap()
}

fn category_id(category: &serde_json::Value) -> Uuid {
    Uuid::parse_str(category["category_id"].as_str().unwrap()).unwrap()
}

fn parameter_id(parameter: &serde_json::Value) -> Uuid {
    Uuid::parse_str(parameter["parameter_type_id"].as_str().unwrap()).unwrap()
}

async fn latest_audit_details(app: &TestApp, resource_id: Uuid, action: &str) -> serde_json::Value {
    sqlx::query_scalar(
        r#"
        SELECT details
        FROM audit_logs
        WHERE resource_id = $1
          AND action = $2
          AND resource_type = 'asset'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(resource_id)
    .bind(action)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
