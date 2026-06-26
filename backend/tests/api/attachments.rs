use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn upload_and_claim_attachments_when_creating_asset_and_inventory_item() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Attachment Create Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let asset_upload = upload(&app, laboratory_id, "asset-note.txt", b"asset file").await;
    let inventory_upload =
        upload(&app, laboratory_id, "inventory-note.txt", b"inventory file").await;

    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Attachment Asset",
                "default_unit_id": unit_id,
                "attachments": [
                    {
                        "upload_id": upload_id(&asset_upload),
                        "display_name": "Asset Manual",
                        "visibility": "internal"
                    }
                ],
                "inventory_items": [
                    {
                        "batch_number": "ATTACH-BATCH",
                        "quantity_on_hand": 2,
                        "attachments": [
                            {
                                "upload_id": upload_id(&inventory_upload),
                                "display_name": "Inventory Receipt",
                                "visibility": "public"
                            }
                        ]
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = value_uuid(&asset["asset_id"]);
    let inventory_item_id = value_uuid(&asset["inventory_items"][0]["inventory_item_id"]);

    let response = app.get_asset_attachments(asset_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let asset_attachments: serde_json::Value = response.json().await.unwrap();
    assert_eq!(asset_attachments.as_array().unwrap().len(), 1);
    assert_eq!(asset_attachments[0]["display_name"], "Asset Manual");
    assert_eq!(
        asset_attachments[0]["sha256_hex"],
        asset_upload["sha256_hex"]
    );

    let response = app.get_inventory_item_attachments(inventory_item_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let inventory_attachments: serde_json::Value = response.json().await.unwrap();
    assert_eq!(inventory_attachments.as_array().unwrap().len(), 1);
    assert_eq!(
        inventory_attachments[0]["display_name"],
        "Inventory Receipt"
    );
    assert_eq!(inventory_attachments[0]["visibility"], "public");

    let direct_inventory_upload = upload(
        &app,
        laboratory_id,
        "inventory-direct.txt",
        b"direct inventory",
    )
    .await;
    let response = app
        .post_inventory_item_attachment(
            inventory_item_id,
            &serde_json::json!({
                "upload_id": upload_id(&direct_inventory_upload),
                "display_name": "Direct Inventory Attachment"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app.get_inventory_item_attachments(inventory_item_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let inventory_attachments: serde_json::Value = response.json().await.unwrap();
    assert_eq!(inventory_attachments.as_array().unwrap().len(), 2);

    let response = app.get_laboratory_attachments(laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let laboratory_attachments: serde_json::Value = response.json().await.unwrap();
    assert_eq!(laboratory_attachments["total"], 3);
}

#[tokio::test]
async fn manage_download_delete_and_filter_attachments_by_laboratory_permissions() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Attachment Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Attachment Other Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let response = app
        .post_asset(
            other_laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Shared Attachment Asset",
                "default_unit_id": unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = value_uuid(&asset["asset_id"]);

    let internal_upload = upload(&app, other_laboratory_id, "internal.txt", b"secret").await;
    let response = app
        .post_asset_attachment(
            asset_id,
            &serde_json::json!({
                "upload_id": upload_id(&internal_upload),
                "display_name": "Internal Spec",
                "description": "before",
                "visibility": "internal"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let internal_attachment: serde_json::Value = response.json().await.unwrap();
    let internal_attachment_id = value_uuid(&internal_attachment["attachment_id"]);

    let response = app
        .patch_attachment(
            internal_attachment_id,
            &serde_json::json!({
                "display_name": "Internal Spec Updated",
                "description": null
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["display_name"], "Internal Spec Updated");
    assert!(updated["description"].is_null());

    let response = app.download_attachment(internal_attachment_id).await;
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(&response.bytes().await.unwrap()[..], b"secret");

    let public_upload = upload(&app, other_laboratory_id, "public.txt", b"public").await;
    let response = app
        .post_asset_attachment(
            asset_id,
            &serde_json::json!({
                "upload_id": upload_id(&public_upload),
                "visibility": "public"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let public_attachment: serde_json::Value = response.json().await.unwrap();
    let public_attachment_id = value_uuid(&public_attachment["attachment_id"]);

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.get_asset_attachments(asset_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let visible: serde_json::Value = response.json().await.unwrap();
    assert_eq!(visible.as_array().unwrap().len(), 1);
    assert_eq!(
        value_uuid(&visible[0]["attachment_id"]),
        public_attachment_id
    );

    let response = app.get_attachment(internal_attachment_id).await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app.download_attachment(public_attachment_id).await;
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(&response.bytes().await.unwrap()[..], b"public");

    let response = app
        .post_asset_attachment(
            asset_id,
            &serde_json::json!({
                "upload_id": upload_id(&public_upload)
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app.delete_attachment(internal_attachment_id).await;
    assert_eq!(response.status().as_u16(), 204);
    let response = app.get_attachment(internal_attachment_id).await;
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn delete_unconsumed_attachment_uploads_only_for_upload_owner() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Attachment Upload Delete Lab").await;
    let unit_id = app.unit_id("pcs").await;
    app.test_user.login(&app).await;

    let upload = upload(&app, laboratory_id, "remove-me.txt", b"remove me").await;
    let upload_id = upload_id(&upload);
    let response = app.delete_attachment_upload(upload_id).await;
    assert_eq!(response.status().as_u16(), 204);
    let response = app.delete_attachment_upload(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 404);

    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Deleted Upload Asset",
                "default_unit_id": unit_id,
                "attachments": [
                    {
                        "upload_id": upload_id,
                        "display_name": "Deleted Upload",
                        "visibility": "internal"
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let owned_by_super_admin = upload(&app, laboratory_id, "owned.txt", b"owned").await;
    let owned_by_super_admin_id = upload_id(&owned_by_super_admin);
    let regular_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.delete_attachment_upload(owned_by_super_admin_id).await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Consumed Upload Asset",
                "default_unit_id": unit_id,
                "attachments": [
                    {
                        "upload_id": owned_by_super_admin_id,
                        "display_name": "Consumed Upload",
                        "visibility": "internal"
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app.delete_attachment_upload(owned_by_super_admin_id).await;
    assert_eq!(response.status().as_u16(), 409);
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
