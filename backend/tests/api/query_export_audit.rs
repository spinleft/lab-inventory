use crate::helpers::{TestUser, spawn_app};
use reqwest::header::CONTENT_TYPE;
use uuid::Uuid;

#[tokio::test]
async fn list_endpoints_support_pagination_search_filters_and_sensitive_search_boundaries() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("Query Owner Lab").await;
    let other_lab = app.create_laboratory("Query Other Lab").await;
    let pcs = app.unit_id("pcs").await;
    let owner = TestUser::generate_with_group("user", Some(owner_lab));
    let viewer = TestUser::generate_with_group("user", Some(other_lab));
    app.store_user(&owner).await;
    app.store_user(&viewer).await;

    app.test_user.login(&app).await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "laboratory_id": owner_lab,
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Query LaserScope",
            "model": "QL-100",
            "default_unit_id": pcs,
            "public_notes": "public query item"
        }))
        .await
        .json()
        .await
        .unwrap();
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let inventory_item: serde_json::Value = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": asset_id,
                "serial_number": "QSN-001",
                "internal_notes": "sensitive serial note"
            }),
            "query-inventory",
        )
        .await
        .json()
        .await
        .unwrap();
    app.post_maintenance_record(&serde_json::json!({
        "asset_id": asset_id,
        "maintenance_type": "inspection",
        "maintained_at": "2026-05-01T00:00:00Z",
        "description": "beam alignment verification"
    }))
    .await;

    let response = app.get_api_path("/assets?q=QL-100&limit=1&offset=0").await;
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status.as_u16(), 200, "{body}");
    let assets: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(assets["limit"], 1);
    assert_eq!(assets["offset"], 0);
    assert_eq!(assets["total"], 1);
    assert_eq!(assets["items"][0]["asset_id"], asset["asset_id"]);

    owner.login(&app).await;
    let response = app
        .get_api_path("/inventory-items?q=QSN-001&tracking_mode=serialized")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(items["total"], 1);
    assert_eq!(items["items"][0]["serial_number"], "QSN-001");

    viewer.login(&app).await;
    let response = app.get_api_path("/inventory-items?q=QSN-001").await;
    assert_eq!(response.status().as_u16(), 200);
    let hidden_items: serde_json::Value = response.json().await.unwrap();
    assert_eq!(hidden_items["total"], 0);

    owner.login(&app).await;
    let response = app
        .get_api_path(&format!(
            "/maintenance-records?q=beam&laboratory_id={owner_lab}"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let records: serde_json::Value = response.json().await.unwrap();
    assert_eq!(records["total"], 1);
    assert_eq!(
        records["items"][0]["inventory_item_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        records["items"][0]["asset_id"],
        serde_json::json!(asset_id.to_string())
    );
    assert_eq!(
        inventory_item["inventory_item_id"].as_str().unwrap(),
        sqlx::query_scalar::<_, String>(
            "SELECT inventory_item_id::text FROM asset_inventory_items WHERE serial_number = 'QSN-001'"
        )
        .fetch_one(&app.db_pool)
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn audit_logs_are_permissioned_and_filterable() {
    let app = spawn_app().await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 401);

    let laboratory_id = app.create_laboratory("Audit Lab").await;
    let pcs = app.unit_id("pcs").await;
    let lab_admin = TestUser::generate_with_group("lab_admin", Some(laboratory_id));
    let user = TestUser::generate_with_group("user", Some(laboratory_id));
    app.store_user(&lab_admin).await;
    app.store_user(&user).await;

    lab_admin.login(&app).await;
    let asset: serde_json::Value = app
        .post_asset(&serde_json::json!({
            "asset_kind": "equipment",
            "tracking_mode": "serialized",
            "name": "Audit Analyzer",
            "default_unit_id": pcs
        }))
        .await
        .json()
        .await
        .unwrap();
    let asset_id = asset["asset_id"].as_str().unwrap();

    let response = app
        .get_api_path(&format!(
            "/audit-logs?resource_type=asset&resource_id={asset_id}&action=create"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let logs: serde_json::Value = response.json().await.unwrap();
    assert_eq!(logs["total"], 1);
    assert_eq!(logs["items"][0]["resource_type"], "asset");
    assert_eq!(
        logs["items"][0]["target_laboratory_id"],
        laboratory_id.to_string()
    );

    user.login(&app).await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let response = app
        .get_api_path(&format!(
            "/audit-logs?target_laboratory_id={laboratory_id}&resource_type=asset"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let system_logs: serde_json::Value = response.json().await.unwrap();
    assert!(system_logs["total"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn csv_exports_reuse_filters_and_hide_cross_laboratory_sensitive_fields() {
    let app = spawn_app().await;
    let owner_lab = app.create_laboratory("CSV Owner Lab").await;
    let other_lab = app.create_laboratory("CSV Other Lab").await;
    let pcs = app.unit_id("pcs").await;
    let owner = TestUser::generate_with_group("user", Some(owner_lab));
    let viewer = TestUser::generate_with_group("user", Some(other_lab));
    app.store_user(&owner).await;
    app.store_user(&viewer).await;

    app.test_user.login(&app).await;
    let location: serde_json::Value = app
        .post_location(&serde_json::json!({
            "laboratory_id": owner_lab,
            "name": "CSV Private Shelf"
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
            "name": "CSV Analyzer",
            "model": "CSV-100",
            "default_unit_id": pcs,
            "public_notes": "public csv asset",
            "internal_notes": "private csv asset"
        }))
        .await
        .json()
        .await
        .unwrap();
    let asset_id: Uuid = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let inventory_item: serde_json::Value = app
        .post_inventory_item(
            &serde_json::json!({
                "asset_id": asset_id,
                "serial_number": "CSV-SN-1",
                "location_id": location["location_id"],
                "is_cross_lab_borrowable": true,
                "public_notes": "public csv inventory",
                "internal_notes": "private csv inventory"
            }),
            "csv-inventory",
        )
        .await
        .json()
        .await
        .unwrap();
    let inventory_item_id: Uuid = inventory_item["inventory_item_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    app.post_maintenance_record(&serde_json::json!({
        "asset_id": asset_id,
        "maintenance_type": "inspection",
        "maintained_at": "2026-05-01T00:00:00Z",
        "description": "csvmaintenance check",
        "public_notes": "public csv maintenance",
        "internal_notes": "private csv maintenance"
    }))
    .await;

    viewer.login(&app).await;
    app.post_borrow_request(
        &serde_json::json!({
            "inventory_item_id": inventory_item_id,
            "requested_quantity": 1.0,
            "purpose": "csvborrow request"
        }),
        "csv-borrow-request",
    )
    .await;

    let response = app
        .get_api_path(&format!(
            "/exports/inventory-items.csv?laboratory_id={owner_lab}"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    let csv = response.text().await.unwrap();
    assert!(csv.contains("CSV Analyzer"));
    assert!(csv.contains("public csv inventory"));
    assert!(!csv.contains("CSV-SN-1"));
    assert!(!csv.contains("CSV Private Shelf"));
    assert!(!csv.contains("private csv inventory"));

    let response = app
        .get_api_path("/exports/borrow-requests.csv?q=csvborrow")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let csv = response.text().await.unwrap();
    assert!(csv.contains("CSV Analyzer"));
    assert!(csv.contains("csvborrow request"));

    let response = app
        .get_api_path("/exports/maintenance-records.csv?q=csvmaintenance")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let csv = response.text().await.unwrap();
    assert!(csv.contains("csvmaintenance check"));
    assert!(csv.contains("public csv maintenance"));
    assert!(!csv.contains("private csv maintenance"));

    owner.login(&app).await;
    let response = app.get_api_path("/exports/assets.csv?q=CSV-100").await;
    assert_eq!(response.status().as_u16(), 200);
    let csv = response.text().await.unwrap();
    assert!(csv.contains("CSV Analyzer"));
    assert!(csv.contains("private csv asset"));
}
