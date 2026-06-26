use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn serialized_inventory_items_can_be_bulk_created_with_explicit_or_default_serials() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Serialized Inventory Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let first_asset_id = create_asset(
        &app,
        laboratory_id,
        unit_id,
        "serialized",
        "Signal Generator",
    )
    .await;
    let response = app
        .post_inventory_items(first_asset_id, &serde_json::json!({ "count": 2 }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(items.as_array().unwrap().len(), 2);
    assert_eq!(items[0]["serial_number"], "#1");
    assert_eq!(items[1]["serial_number"], "#2");

    let response = app
        .post_inventory_items(
            first_asset_id,
            &serde_json::json!({ "serial_numbers": ["SN-A", "SN-B"] }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app
        .post_inventory_items(first_asset_id, &serde_json::json!({ "count": 1 }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(items[0]["serial_number"], "#3");

    let duplicate = app
        .post_inventory_items(
            first_asset_id,
            &serde_json::json!({ "serial_numbers": ["#1"] }),
        )
        .await;
    assert_eq!(duplicate.status().as_u16(), 409);

    let second_asset_id =
        create_asset(&app, laboratory_id, unit_id, "serialized", "Power Supply").await;
    let response = app
        .post_inventory_items(second_asset_id, &serde_json::json!({ "count": 1 }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(items[0]["serial_number"], "#1");
}

#[tokio::test]
async fn serialized_inventory_items_can_claim_attachments_per_serial_item() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Serialized Attachment Lab").await;
    let other_laboratory_id = app
        .create_laboratory("Other Serialized Attachment Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let asset_id = create_asset(
        &app,
        laboratory_id,
        unit_id,
        "serialized",
        "Attached Serial Asset",
    )
    .await;
    let first_upload = upload(&app, laboratory_id, "serial-a.txt", b"serial a").await;
    let second_upload = upload(&app, laboratory_id, "serial-b.txt", b"serial b").await;

    let response = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "serial_items": [
                    {
                        "serial_number": "SN-A",
                        "attachments": [
                            {
                                "upload_id": upload_id(&first_upload),
                                "display_name": "Serial A Manual",
                                "visibility": "internal"
                            }
                        ]
                    },
                    {
                        "serial_number": "SN-B",
                        "attachments": [
                            {
                                "upload_id": upload_id(&second_upload),
                                "display_name": "Serial B Manual",
                                "visibility": "public"
                            }
                        ]
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(items.as_array().unwrap().len(), 2);

    let first_response = app
        .get_inventory_item_attachments(value_uuid(&items[0]["inventory_item_id"]))
        .await;
    assert_eq!(first_response.status().as_u16(), 200);
    let first_attachments: serde_json::Value = first_response.json().await.unwrap();
    assert_eq!(first_attachments.as_array().unwrap().len(), 1);
    assert_eq!(first_attachments[0]["display_name"], "Serial A Manual");
    assert_eq!(first_attachments[0]["visibility"], "internal");
    assert_eq!(
        first_attachments[0]["sha256_hex"],
        first_upload["sha256_hex"]
    );

    let second_response = app
        .get_inventory_item_attachments(value_uuid(&items[1]["inventory_item_id"]))
        .await;
    assert_eq!(second_response.status().as_u16(), 200);
    let second_attachments: serde_json::Value = second_response.json().await.unwrap();
    assert_eq!(second_attachments.as_array().unwrap().len(), 1);
    assert_eq!(second_attachments[0]["display_name"], "Serial B Manual");
    assert_eq!(second_attachments[0]["visibility"], "public");
    assert_eq!(
        second_attachments[0]["sha256_hex"],
        second_upload["sha256_hex"]
    );

    let duplicate_upload = upload(&app, laboratory_id, "duplicate.txt", b"duplicate").await;
    let response = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "serial_items": [
                    {
                        "serial_number": "SN-C",
                        "attachments": [{ "upload_id": upload_id(&duplicate_upload) }]
                    },
                    {
                        "serial_number": "SN-D",
                        "attachments": [{ "upload_id": upload_id(&duplicate_upload) }]
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "serial_items": [
                    {
                        "serial_number": "SN-E",
                        "attachments": [{ "upload_id": Uuid::new_v4() }]
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let other_upload = upload(&app, other_laboratory_id, "other.txt", b"other").await;
    let response = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "serial_items": [
                    {
                        "serial_number": "SN-F",
                        "attachments": [{ "upload_id": upload_id(&other_upload) }]
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn quantity_inventory_items_can_be_created_queried_updated_and_batch_updated() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Quantity Inventory Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let other_unit_id = app.unit_id("cm").await;
    app.test_user.login(&app).await;

    let location_id = create_location(&app, laboratory_id, "Shelf A", "shelf_a").await;
    let asset_id = create_asset(&app, laboratory_id, unit_id, "quantity", "Chemical Reagent").await;
    let first_item = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({
            "batch_number": "B-001",
            "quantity_on_hand": 10,
            "quantity_unit_id": unit_id,
            "location_id": location_id,
            "status": "available",
            "public_notes": "rack note"
        }),
    )
    .await;
    assert_eq!(first_item["quantity_unit_id"], unit_id.to_string());
    let first_item_id = inventory_item_id(&first_item);

    let mismatched_unit = app
        .post_inventory_items(
            asset_id,
            &serde_json::json!({
                "batch_number": "B-UNIT",
                "quantity_on_hand": 1,
                "quantity_unit_id": other_unit_id
            }),
        )
        .await;
    assert_eq!(mismatched_unit.status().as_u16(), 400);

    let response = app
        .get_inventory_items_with_query(
            laboratory_id,
            &format!("keyword=Chemical&batch_number=B-001&location_id={location_id}"),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let list: serde_json::Value = response.json().await.unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(
        list["items"][0]["inventory_item_id"],
        first_item_id.to_string()
    );
    assert_eq!(list["items"][0]["asset"]["name"], "Chemical Reagent");

    let response = app.get_inventory_item(first_item_id).await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app
        .patch_inventory_item(
            first_item_id,
            &serde_json::json!({
                "quantity_on_hand": 8,
                "status": "reserved",
                "batch_number": "B-002",
                "location_id": null,
                "public_notes": "updated"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["quantity_on_hand"], 8.0);
    assert_eq!(updated["status"], "reserved");
    assert_eq!(updated["batch_number"], "B-002");
    assert!(updated["location_id"].is_null());

    let second_item = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({
            "batch_number": "B-003",
            "quantity_on_hand": 5,
            "quantity_unit_id": unit_id
        }),
    )
    .await;
    let second_item_id = inventory_item_id(&second_item);
    let response = app
        .patch_inventory_items_batch(&serde_json::json!({
            "inventory_item_ids": [first_item_id, second_item_id],
            "status": "retired",
            "public_notes": null
        }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let batch: serde_json::Value = response.json().await.unwrap();
    assert_eq!(batch.as_array().unwrap().len(), 2);
    assert!(
        batch
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["status"] == "retired")
    );

    let invalid = app
        .patch_inventory_item(
            first_item_id,
            &serde_json::json!({ "serial_number": "NOT-ALLOWED" }),
        )
        .await;
    assert_eq!(invalid.status().as_u16(), 400);
}

#[tokio::test]
async fn quantity_inventory_items_can_be_split_into_new_or_existing_aggregates() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Split Inventory Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let source_location_id =
        create_location(&app, laboratory_id, "Source Shelf", "source_shelf").await;
    let target_location_id =
        create_location(&app, laboratory_id, "Target Shelf", "target_shelf").await;
    let asset_id = create_asset(&app, laboratory_id, unit_id, "quantity", "Split Reagent").await;
    let source = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({
            "batch_number": "SPLIT",
            "quantity_on_hand": 10,
            "quantity_unit_id": unit_id,
            "location_id": source_location_id
        }),
    )
    .await;
    let source_id = inventory_item_id(&source);

    let response = app
        .split_inventory_item(
            source_id,
            &serde_json::json!({
                "quantity": 3,
                "batch_number": "SPLIT",
                "location_id": target_location_id,
                "status": "reserved"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let split: serde_json::Value = response.json().await.unwrap();
    assert_eq!(split["source"]["quantity_on_hand"], 7.0);
    assert_eq!(split["target"]["quantity_on_hand"], 3.0);
    let target_id = inventory_item_id(&split["target"]);

    let response = app
        .split_inventory_item(
            source_id,
            &serde_json::json!({
                "quantity": 2,
                "batch_number": "SPLIT",
                "location_id": target_location_id,
                "status": "reserved"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let split: serde_json::Value = response.json().await.unwrap();
    assert_eq!(split["source"]["quantity_on_hand"], 5.0);
    assert_eq!(split["target"]["inventory_item_id"], target_id.to_string());
    assert_eq!(split["target"]["quantity_on_hand"], 5.0);

    let response = app
        .split_inventory_item(source_id, &serde_json::json!({ "quantity": 99 }))
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn quantity_inventory_items_merge_strictly_and_reject_mismatched_aggregates() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Merge Inventory Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    sqlx::query("DROP INDEX idx_asset_inventory_items_unique_quantity_aggregate")
        .execute(&app.db_pool)
        .await
        .unwrap();

    let asset_id = create_asset(&app, laboratory_id, unit_id, "quantity", "Merge Reagent").await;
    let target_id = insert_quantity_item(
        &app,
        laboratory_id,
        asset_id,
        unit_id,
        Some("MERGE"),
        2.0,
        1.0,
    )
    .await;
    let source_id = insert_quantity_item(
        &app,
        laboratory_id,
        asset_id,
        unit_id,
        Some("MERGE"),
        3.0,
        1.0,
    )
    .await;

    let response = app
        .merge_inventory_items(&serde_json::json!({
            "target_inventory_item_id": target_id,
            "source_inventory_item_ids": [source_id]
        }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let merged: serde_json::Value = response.json().await.unwrap();
    assert_eq!(merged["inventory_item_id"], target_id.to_string());
    assert_eq!(merged["quantity_on_hand"], 5.0);
    assert_eq!(merged["quantity_allocated"], 2.0);
    let source_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM asset_inventory_items WHERE inventory_item_id = $1",
    )
    .bind(source_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(source_count, 0);

    let mismatched_source_id = insert_quantity_item(
        &app,
        laboratory_id,
        asset_id,
        unit_id,
        Some("OTHER"),
        1.0,
        0.0,
    )
    .await;
    let response = app
        .merge_inventory_items(&serde_json::json!({
            "target_inventory_item_id": target_id,
            "source_inventory_item_ids": [mismatched_source_id]
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn inventory_item_delete_rejects_allocated_items_and_batch_delete_removes_unallocated_items()
{
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Delete Inventory Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let asset_id = create_asset(&app, laboratory_id, unit_id, "quantity", "Delete Reagent").await;
    let unallocated = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({
            "batch_number": "DEL-1",
            "quantity_on_hand": 4
        }),
    )
    .await;
    let unallocated_id = inventory_item_id(&unallocated);
    let response = app.delete_inventory_item(unallocated_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let allocated = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({
            "batch_number": "DEL-2",
            "quantity_on_hand": 4,
            "quantity_allocated": 1
        }),
    )
    .await;
    let allocated_id = inventory_item_id(&allocated);
    let response = app.delete_inventory_item(allocated_id).await;
    assert_eq!(response.status().as_u16(), 409);

    let first = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({ "batch_number": "DEL-3", "quantity_on_hand": 1 }),
    )
    .await;
    let second = create_quantity_item(
        &app,
        asset_id,
        serde_json::json!({ "batch_number": "DEL-4", "quantity_on_hand": 1 }),
    )
    .await;
    let response = app
        .batch_delete_inventory_items(&serde_json::json!({
            "inventory_item_ids": [inventory_item_id(&first), inventory_item_id(&second)]
        }))
        .await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn inventory_item_permissions_follow_laboratory_scope() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Inventory Permission Own").await;
    let other_laboratory_id = app.create_laboratory("Inventory Permission Other").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;
    let other_asset_id = create_asset(
        &app,
        other_laboratory_id,
        unit_id,
        "quantity",
        "Other Lab Asset",
    )
    .await;
    let other_item = create_quantity_item(
        &app,
        other_asset_id,
        serde_json::json!({
            "batch_number": "OTHER",
            "quantity_on_hand": 5,
            "internal_notes": "Other lab internal notes"
        }),
    )
    .await;
    let other_item_id = inventory_item_id(&other_item);

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;
    let own_asset_id = create_asset(
        &app,
        own_laboratory_id,
        unit_id,
        "quantity",
        "Own Lab Asset",
    )
    .await;
    let own_item = create_quantity_item(
        &app,
        own_asset_id,
        serde_json::json!({ "batch_number": "OWN", "quantity_on_hand": 2 }),
    )
    .await;
    let own_item_id = inventory_item_id(&own_item);
    let response = app.get_inventory_items(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let response = app.get_inventory_item(own_item_id).await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app.get_inventory_items(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["items"][0]["inventory_item_id"],
        other_item_id.to_string()
    );
    assert!(body["items"][0]["internal_notes"].is_null());

    let response = app.get_inventory_item(other_item_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["internal_notes"].is_null());

    let response = app
        .post_inventory_items(
            other_asset_id,
            &serde_json::json!({ "batch_number": "DENY", "quantity_on_hand": 1 }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .patch_inventory_item(own_item_id, &serde_json::json!({ "status": "retired" }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

async fn create_asset(
    app: &TestApp,
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
                "default_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(asset["asset_id"].as_str().unwrap()).unwrap()
}

async fn create_location(app: &TestApp, laboratory_id: Uuid, name: &str, code: &str) -> Uuid {
    let response = app
        .post_location(
            laboratory_id,
            &serde_json::json!({
                "name": name,
                "code": code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let location: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(location["location_id"].as_str().unwrap()).unwrap()
}

async fn create_quantity_item(
    app: &TestApp,
    asset_id: Uuid,
    body: serde_json::Value,
) -> serde_json::Value {
    let response = app.post_inventory_items(asset_id, &body).await;
    assert_eq!(response.status().as_u16(), 201);
    let items: serde_json::Value = response.json().await.unwrap();
    items[0].clone()
}

async fn insert_quantity_item(
    app: &TestApp,
    laboratory_id: Uuid,
    asset_id: Uuid,
    unit_id: Uuid,
    batch_number: Option<&str>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            batch_number,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id,
            status
        )
        VALUES ($1, $2, $3, 'quantity', $4, $5, $6, $7, 'available')
        RETURNING inventory_item_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(batch_number)
    .bind(quantity_on_hand)
    .bind(quantity_allocated)
    .bind(unit_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

fn inventory_item_id(item: &serde_json::Value) -> Uuid {
    Uuid::parse_str(item["inventory_item_id"].as_str().unwrap()).unwrap()
}

async fn upload(
    app: &TestApp,
    laboratory_id: Uuid,
    file_name: &str,
    bytes: &[u8],
) -> serde_json::Value {
    let response = app
        .upload_attachment(laboratory_id, file_name, "text/plain", bytes.to_vec())
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

fn upload_id(upload: &serde_json::Value) -> Uuid {
    value_uuid(&upload["upload_id"])
}

fn value_uuid(value: &serde_json::Value) -> Uuid {
    Uuid::parse_str(value.as_str().unwrap()).unwrap()
}
