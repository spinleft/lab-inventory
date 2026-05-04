use crate::helpers::{TestUser, spawn_app};
use chrono::{Duration, Utc};
use uuid::Uuid;

async fn create_quantity_asset(
    app: &crate::helpers::TestApp,
    laboratory_id: Uuid,
    name: &str,
) -> serde_json::Value {
    let meter = app.unit_id("m").await;
    app.post_asset(&serde_json::json!({
        "laboratory_id": laboratory_id,
        "asset_kind": "material",
        "tracking_mode": "quantity",
        "name": name,
        "default_unit_id": meter,
        "public_notes": "public asset",
        "internal_notes": "private asset"
    }))
    .await
    .json()
    .await
    .unwrap()
}

async fn create_serialized_inventory(app: &crate::helpers::TestApp, laboratory_id: Uuid) -> Uuid {
    let pcs = app.unit_id("pcs").await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": laboratory_id,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Maintained Analyzer",
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
                "serial_number": "MAINT-001"
            }),
            "maintenance-serialized-item",
        )
        .await
        .json()
        .await
        .unwrap();
    item["inventory_item_id"].as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn maintenance_records_are_permissioned_and_hide_internal_fields_cross_lab() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Maintenance Owner Lab").await;
    let other_lab = app.create_laboratory("Maintenance Other Lab").await;
    app.test_user.login(&app).await;
    let asset = create_quantity_asset(&app, owner_lab, "Pump Oil").await;
    let other_asset = create_quantity_asset(&app, other_lab, "Other Pump Oil").await;
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let other_asset_id: Uuid = other_asset["asset_id"].as_str().unwrap().parse().unwrap();
    app.post_logout().await;

    let body = serde_json::json!({
        "asset_id": asset_id,
        "maintenance_type": "inspection",
        "maintained_at": "2026-05-01T00:00:00Z",
        "description": "checked optics",
        "public_notes": "public summary",
        "internal_notes": "private calibration notes"
    });
    let response = app.post_maintenance_record(&body).await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_user_type("guest", Some(owner_lab));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app.post_maintenance_record(&body).await;
    assert_eq!(response.status().as_u16(), 403);

    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    app.store_user(&owner).await;
    owner.login(&app).await;
    let response = app
        .post_maintenance_record(&serde_json::json!({
            "asset_id": other_asset_id,
            "maintenance_type": "inspection",
            "maintained_at": "2026-05-01T00:00:00Z",
            "description": "forbidden"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app.post_maintenance_record(&body).await;
    assert_eq!(response.status().as_u16(), 201);
    let record: serde_json::Value = response.json().await.unwrap();
    assert_eq!(record["internal_notes"], "private calibration notes");
    assert!(record.get("cost").is_none());
    let record_id: Uuid = record["maintenance_record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let updated: serde_json::Value = app
        .patch_maintenance_record(
            record_id,
            &serde_json::json!({
                "description": "checked and cleaned optics"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(updated["description"], "checked and cleaned optics");

    let viewer = TestUser::generate_with_user_type("user", Some(other_lab));
    app.store_user(&viewer).await;
    viewer.login(&app).await;
    let fetched: serde_json::Value = app
        .get_maintenance_record(record_id)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(fetched["public_notes"], "public summary");
    assert!(fetched["internal_notes"].is_null());

    let records: serde_json::Value = app.get_maintenance_records().await.json().await.unwrap();
    let listed = records["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["maintenance_record_id"] == record_id.to_string())
        .unwrap();
    assert!(listed["internal_notes"].is_null());

    owner.login(&app).await;
    let response = app.delete_maintenance_record(record_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn attachments_are_metadata_only_and_respect_visibility() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Attachment Owner Lab").await;
    let other_lab = app.create_laboratory("Attachment Other Lab").await;
    app.test_user.login(&app).await;
    let asset = create_quantity_asset(&app, owner_lab, "Attachment Asset").await;
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();

    let owner = TestUser::generate_with_user_type("user", Some(owner_lab));
    let viewer = TestUser::generate_with_user_type("user", Some(other_lab));
    let guest = TestUser::generate_with_user_type("guest", Some(owner_lab));
    app.store_user(&owner).await;
    app.store_user(&viewer).await;
    app.store_user(&guest).await;

    guest.login(&app).await;
    let response = app
        .post_attachment(&serde_json::json!({
            "resource_type": "asset",
            "resource_id": asset_id,
            "file_name": "manual.pdf",
            "mime_type": "application/pdf",
            "file_size_bytes": 1000,
            "storage_url": "local://manual.pdf",
            "visibility": "public"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    owner.login(&app).await;
    let public_attachment: serde_json::Value = app
        .post_attachment(&serde_json::json!({
            "resource_type": "asset",
            "resource_id": asset_id,
            "file_name": "manual.pdf",
            "mime_type": "application/pdf",
            "file_size_bytes": 1000,
            "storage_url": "local://manual.pdf",
            "visibility": "public"
        }))
        .await
        .json()
        .await
        .unwrap();
    let internal_attachment: serde_json::Value = app
        .post_attachment(&serde_json::json!({
            "resource_type": "asset",
            "resource_id": asset_id,
            "file_name": "internal.txt",
            "mime_type": "text/plain",
            "file_size_bytes": 32,
            "storage_url": "local://internal.txt",
            "visibility": "internal"
        }))
        .await
        .json()
        .await
        .unwrap();
    let internal_attachment_id: Uuid = internal_attachment["attachment_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    viewer.login(&app).await;
    let attachments: serde_json::Value = app.get_attachments().await.json().await.unwrap();
    assert!(
        attachments["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|attachment| {
                attachment["attachment_id"] == public_attachment["attachment_id"]
                    && attachment["file_name"] == "manual.pdf"
            })
    );
    assert!(
        attachments["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|attachment| {
                attachment["attachment_id"] != internal_attachment["attachment_id"]
            })
    );

    owner.login(&app).await;
    let response = app.delete_attachment(internal_attachment_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn maintenance_schedules_generate_due_and_overdue_alerts() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Schedule Owner Lab").await;
    let other_lab = app.create_laboratory("Schedule Other Lab").await;
    app.test_user.login(&app).await;
    let asset = create_quantity_asset(&app, owner_lab, "Scheduled Oil").await;
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let inventory_item_id = create_serialized_inventory(&app, owner_lab).await;

    let owner = TestUser::generate_with_user_type("maintainer", Some(owner_lab));
    let viewer = TestUser::generate_with_user_type("user", Some(other_lab));
    let guest = TestUser::generate_with_user_type("guest", Some(owner_lab));
    app.store_user(&owner).await;
    app.store_user(&viewer).await;
    app.store_user(&guest).await;

    guest.login(&app).await;
    let response = app
        .post_maintenance_schedule(&serde_json::json!({
            "asset_id": asset_id,
            "schedule_name": "guest schedule",
            "interval_days": 30,
            "next_maintenance_at": "2026-05-03T00:00:00Z",
            "remind_before_days": 7
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    owner.login(&app).await;
    let due_soon_at = (Utc::now() + Duration::days(3)).to_rfc3339();
    let due: serde_json::Value = app
        .post_maintenance_schedule(&serde_json::json!({
            "asset_id": asset_id,
            "schedule_name": "oil check",
            "interval_days": 30,
            "next_maintenance_at": due_soon_at,
            "remind_before_days": 7,
            "public_notes": "public schedule",
            "internal_notes": "private schedule"
        }))
        .await
        .json()
        .await
        .unwrap();
    let overdue: serde_json::Value = app
        .post_maintenance_schedule(&serde_json::json!({
            "inventory_item_id": inventory_item_id,
            "schedule_name": "analyzer calibration",
            "interval_days": 90,
            "next_maintenance_at": "2000-01-01T00:00:00Z",
            "remind_before_days": 14
        }))
        .await
        .json()
        .await
        .unwrap();
    app.post_maintenance_schedule(&serde_json::json!({
        "asset_id": asset_id,
        "schedule_name": "disabled schedule",
        "interval_days": 30,
        "next_maintenance_at": "2000-01-01T00:00:00Z",
        "remind_before_days": 7,
        "is_active": false
    }))
    .await;
    let schedules: serde_json::Value = app.get_maintenance_schedules().await.json().await.unwrap();
    assert_eq!(schedules["items"].as_array().unwrap().len(), 3);

    let alerts: serde_json::Value = app.get_maintenance_alerts().await.json().await.unwrap();
    assert!(alerts.as_array().unwrap().iter().any(|alert| {
        alert["maintenance_schedule_id"] == due["maintenance_schedule_id"]
            && alert["alert_kind"] == "due_soon"
    }));
    assert!(alerts.as_array().unwrap().iter().any(|alert| {
        alert["maintenance_schedule_id"] == overdue["maintenance_schedule_id"]
            && alert["alert_kind"] == "overdue"
    }));
    assert!(
        alerts
            .as_array()
            .unwrap()
            .iter()
            .all(|alert| { alert["schedule_name"] != "disabled schedule" })
    );

    viewer.login(&app).await;
    let alerts: serde_json::Value = app.get_maintenance_alerts().await.json().await.unwrap();
    let due_alert = alerts
        .as_array()
        .unwrap()
        .iter()
        .find(|alert| alert["maintenance_schedule_id"] == due["maintenance_schedule_id"])
        .unwrap();
    assert_eq!(due_alert["public_notes"], "public schedule");
    assert!(due_alert["internal_notes"].is_null());

    owner.login(&app).await;
    let updated: serde_json::Value = app
        .patch_maintenance_schedule(
            overdue["maintenance_schedule_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            &serde_json::json!({ "is_active": false }),
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(updated["is_active"], false);
    let response = app
        .delete_maintenance_schedule(
            due["maintenance_schedule_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
        )
        .await;
    assert_eq!(response.status().as_u16(), 204);
}
