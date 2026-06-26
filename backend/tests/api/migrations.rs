use crate::helpers::spawn_app;

#[tokio::test]
async fn migrations_seed_default_user_types_and_audit_log_table() {
    let app = spawn_app().await;

    let user_types: Vec<String> = sqlx::query_scalar("SELECT name FROM user_types ORDER BY name")
        .fetch_all(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(
        user_types,
        vec!["guest", "lab_admin", "root", "super_admin", "user"]
    );

    let user_type_ids: Vec<String> =
        sqlx::query_scalar("SELECT user_type_id::text FROM user_types ORDER BY name")
            .fetch_all(&app.db_pool)
            .await
            .unwrap();
    for user_type_id in user_type_ids {
        assert!(
            looks_like_versioned_uuid(&user_type_id),
            "{user_type_id} should include UUID version and variant bits"
        );
    }

    let root: (String, Option<uuid::Uuid>) = sqlx::query_as(
        r#"
        SELECT user_types.name, users.laboratory_id
        FROM users
        INNER JOIN user_types USING (user_type_id)
        WHERE users.username = 'root'
        "#,
    )
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(root.0, "root");
    assert!(root.1.is_none());

    let audit_log_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs")
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(audit_log_count, 0);
}

fn looks_like_versioned_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && parts[2]
            .chars()
            .next()
            .is_some_and(|version| ('1'..='8').contains(&version))
        && parts[3]
            .chars()
            .next()
            .is_some_and(|variant| matches!(variant, '8' | '9' | 'a' | 'b'))
}

#[tokio::test]
async fn migrations_create_inventory_foundation_tables_and_seed_units() {
    let app = spawn_app().await;

    let unit_codes: Vec<String> = sqlx::query_scalar("SELECT code FROM units ORDER BY code")
        .fetch_all(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(unit_codes, vec!["cm", "inch", "m", "mm", "pcs"]);

    let required_tables = vec![
        "asset_categories",
        "locations",
        "assets",
        "asset_inventory_items",
        "attachment_uploads",
        "attachments",
        "inventory_transactions",
        "idempotency",
    ];
    for table_name in required_tables {
        let exists: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public'
              AND table_name = $1
            "#,
        )
        .bind(table_name)
        .fetch_optional(&app.db_pool)
        .await
        .unwrap();
        assert!(exists.is_some(), "{table_name} should exist");
    }

    let header_pair_type: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM pg_type WHERE typname = 'header_pair'")
            .fetch_optional(&app.db_pool)
            .await
            .unwrap();
    assert!(header_pair_type.is_some());

    let pg_trgm_extension: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm'")
            .fetch_optional(&app.db_pool)
            .await
            .unwrap();
    assert!(pg_trgm_extension.is_some());

    let legacy_idempotency_keys_table: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_name = 'idempotency_keys'
        "#,
    )
    .fetch_optional(&app.db_pool)
    .await
    .unwrap();
    assert!(legacy_idempotency_keys_table.is_none());

    let nullable_response_columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'idempotency'
          AND column_name IN ('response_status_code', 'response_headers', 'response_body')
          AND is_nullable = 'YES'
        ORDER BY column_name
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        nullable_response_columns,
        vec!["response_body", "response_headers", "response_status_code"]
    );

    let asset_tag_column: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'asset_inventory_items'
          AND column_name = 'asset_tag'
        "#,
    )
    .fetch_optional(&app.db_pool)
    .await
    .unwrap();
    assert!(asset_tag_column.is_none());

    let related_resource_columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'inventory_transactions'
          AND column_name IN ('related_resource_type', 'related_resource_id')
        ORDER BY column_name
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        related_resource_columns,
        vec!["related_resource_id", "related_resource_type"]
    );

    let threshold_columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'assets'
          AND column_name IN ('minimum_stock_quantity', 'minimum_stock_unit_id')
        ORDER BY column_name
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert!(threshold_columns.is_empty());

    let cost_column: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'maintenance_records'
          AND column_name ILIKE '%cost%'
        "#,
    )
    .fetch_optional(&app.db_pool)
    .await
    .unwrap();
    assert!(cost_column.is_none());

    let search_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT indexname
        FROM pg_indexes
        WHERE schemaname = 'public'
          AND indexname IN (
            'idx_assets_search_trgm',
            'idx_asset_inventory_items_search_trgm'
          )
        ORDER BY indexname
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        search_indexes,
        vec![
            "idx_asset_inventory_items_search_trgm",
            "idx_assets_search_trgm"
        ]
    );

    let inventory_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT indexname
        FROM pg_indexes
        WHERE schemaname = 'public'
          AND indexname IN (
            'uq_assets_asset_laboratory',
            'uq_locations_location_laboratory',
            'idx_asset_inventory_items_unique_asset_serial_number',
            'idx_asset_inventory_items_unique_quantity_aggregate',
            'idx_asset_inventory_items_asset_laboratory_id',
            'idx_asset_inventory_items_laboratory_asset_id',
            'idx_asset_inventory_items_laboratory_status',
            'idx_asset_inventory_items_laboratory_batch_number',
            'idx_asset_inventory_items_laboratory_location_id',
            'idx_asset_inventory_items_location_laboratory_id',
            'idx_asset_inventory_items_quantity_unit_id',
            'uq_asset_inventory_items_item_laboratory',
            'uq_assets_asset_laboratory_tracking_mode'
          )
        ORDER BY indexname
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        inventory_indexes,
        vec![
            "idx_asset_inventory_items_asset_laboratory_id",
            "idx_asset_inventory_items_laboratory_asset_id",
            "idx_asset_inventory_items_laboratory_batch_number",
            "idx_asset_inventory_items_laboratory_location_id",
            "idx_asset_inventory_items_laboratory_status",
            "idx_asset_inventory_items_location_laboratory_id",
            "idx_asset_inventory_items_quantity_unit_id",
            "idx_asset_inventory_items_unique_asset_serial_number",
            "idx_asset_inventory_items_unique_quantity_aggregate",
            "uq_asset_inventory_items_item_laboratory",
            "uq_assets_asset_laboratory",
            "uq_assets_asset_laboratory_tracking_mode",
            "uq_locations_location_laboratory"
        ]
    );

    let attachment_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT indexname
        FROM pg_indexes
        WHERE schemaname = 'public'
          AND indexname IN (
            'idx_attachment_uploads_laboratory_active',
            'idx_attachments_active_asset',
            'idx_attachments_active_inventory_item',
            'idx_attachments_asset_laboratory_id',
            'idx_attachments_inventory_item_laboratory_id',
            'idx_attachments_laboratory_created_active',
            'idx_attachments_display_name_trgm'
          )
        ORDER BY indexname
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        attachment_indexes,
        vec![
            "idx_attachment_uploads_laboratory_active",
            "idx_attachments_active_asset",
            "idx_attachments_active_inventory_item",
            "idx_attachments_asset_laboratory_id",
            "idx_attachments_display_name_trgm",
            "idx_attachments_inventory_item_laboratory_id",
            "idx_attachments_laboratory_created_active",
        ]
    );
}

#[tokio::test]
async fn inventory_quantity_constraints_are_enforced_by_the_database() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Constraint Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let asset_id = uuid::Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            tracking_mode,
            name,
            default_unit_id
        )
        VALUES ($1, $2, 'quantity', 'Resistors', $3)
        "#,
    )
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await
    .unwrap();

    let result = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', 2, 3, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn attachment_constraints_are_enforced_by_the_database() {
    let app = spawn_app().await;
    let unit_id = app.unit_id("pcs").await;
    let laboratory_id = app.create_laboratory("Attachment Constraint Lab").await;
    let other_laboratory_id = app
        .create_laboratory("Attachment Other Constraint Lab")
        .await;
    let asset_id = insert_test_asset(&app, laboratory_id, unit_id).await;
    let inventory_item_id =
        insert_quantity_inventory_item(&app, asset_id, laboratory_id, unit_id, Some("ATTACH"))
            .await;

    let legacy_columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'attachments'
          AND column_name IN ('resource_type', 'resource_id', 'file_name', 'storage_url')
        ORDER BY column_name
        "#,
    )
    .fetch_all(&app.db_pool)
    .await
    .unwrap();
    assert!(legacy_columns.is_empty());

    insert_attachment(&app, laboratory_id, Some(asset_id), None)
        .await
        .unwrap();

    let no_target = insert_attachment(&app, laboratory_id, None, None).await;
    assert!(no_target.is_err());

    let two_targets =
        insert_attachment(&app, laboratory_id, Some(asset_id), Some(inventory_item_id)).await;
    assert!(two_targets.is_err());

    let cross_laboratory = insert_attachment(&app, other_laboratory_id, Some(asset_id), None).await;
    assert!(cross_laboratory.is_err());
}

async fn insert_attachment(
    app: &crate::helpers::TestApp,
    laboratory_id: uuid::Uuid,
    asset_id: Option<uuid::Uuid>,
    inventory_item_id: Option<uuid::Uuid>,
) -> Result<(), sqlx::Error> {
    let storage_key = format!(
        "labs/{laboratory_id}/objects/{}/constraint.txt",
        uuid::Uuid::new_v4()
    );
    sqlx::query(
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
        VALUES ($1, $2, $3, $4, 'constraint.txt', 'constraint.txt', 1, $5, 'local', $6)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(laboratory_id)
    .bind(asset_id)
    .bind(inventory_item_id)
    .bind("b".repeat(64))
    .bind(storage_key)
    .execute(&app.db_pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn inventory_item_laboratory_consistency_is_enforced_by_the_database() {
    let app = spawn_app().await;
    let unit_id = app.unit_id("pcs").await;
    let asset_laboratory_id = app.create_laboratory("Asset Lab").await;
    let other_laboratory_id = app.create_laboratory("Other Lab").await;
    let asset_id = insert_test_asset(&app, asset_laboratory_id, unit_id).await;
    let other_location_id = insert_test_location(&app, other_laboratory_id, "other_location").await;

    let asset_mismatch = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', 1, 0, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(other_laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(asset_mismatch.is_err());

    let location_mismatch = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id,
            location_id
        )
        VALUES ($1, $2, $3, 'quantity', 1, 0, $4, $5)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(asset_laboratory_id)
    .bind(unit_id)
    .bind(other_location_id)
    .execute(&app.db_pool)
    .await;
    assert!(location_mismatch.is_err());
}

#[tokio::test]
async fn inventory_item_batch_and_serial_constraints_are_enforced_by_the_database() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Batch Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let asset_id = insert_test_asset(&app, laboratory_id, unit_id).await;

    insert_quantity_inventory_item(&app, asset_id, laboratory_id, unit_id, Some("BATCH-001")).await;
    let duplicate_batch = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            batch_number,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', 'BATCH-001', 1, 0, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(duplicate_batch.is_err());

    let blank_batch = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            batch_number,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', '   ', 1, 0, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(blank_batch.is_err());

    let blank_serial = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            serial_number,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'serialized', '   ', 1, 0, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(blank_serial.is_err());
}

#[tokio::test]
async fn inventory_item_tracking_mode_matches_asset_and_cascades_on_asset_delete() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Tracking Cascade Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let asset_id = insert_test_asset(&app, laboratory_id, unit_id).await;

    let tracking_mismatch = sqlx::query(
        r#"
        INSERT INTO asset_inventory_items (
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            serial_number,
            quantity_on_hand,
            quantity_allocated,
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'serialized', 'SERIAL-001', 1, 0, $4)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(tracking_mismatch.is_err());

    insert_quantity_inventory_item(&app, asset_id, laboratory_id, unit_id, Some("CASCADE")).await;
    sqlx::query("DELETE FROM assets WHERE asset_id = $1")
        .bind(asset_id)
        .execute(&app.db_pool)
        .await
        .unwrap();

    let inventory_item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asset_inventory_items WHERE asset_id = $1")
            .bind(asset_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(inventory_item_count, 0);
}

async fn insert_test_asset(
    app: &crate::helpers::TestApp,
    laboratory_id: uuid::Uuid,
    unit_id: uuid::Uuid,
) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            tracking_mode,
            name,
            default_unit_id
        )
        VALUES ($1, $2, 'quantity', $3, $4)
        RETURNING asset_id
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(laboratory_id)
    .bind(format!("Test Asset {}", uuid::Uuid::new_v4()))
    .bind(unit_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

async fn insert_test_location(
    app: &crate::helpers::TestApp,
    laboratory_id: uuid::Uuid,
    code: &str,
) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO locations (
            location_id,
            laboratory_id,
            name,
            code,
            path,
            depth
        )
        VALUES ($1, $2, $3, $4, $4::text::ltree, 0)
        RETURNING location_id
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(laboratory_id)
    .bind(format!("Location {code}"))
    .bind(code)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

async fn insert_quantity_inventory_item(
    app: &crate::helpers::TestApp,
    asset_id: uuid::Uuid,
    laboratory_id: uuid::Uuid,
    unit_id: uuid::Uuid,
    batch_number: Option<&str>,
) -> uuid::Uuid {
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
            quantity_unit_id
        )
        VALUES ($1, $2, $3, 'quantity', $4, 1, 0, $5)
        RETURNING inventory_item_id
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(batch_number)
    .bind(unit_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
