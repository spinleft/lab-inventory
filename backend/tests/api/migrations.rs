use crate::helpers::spawn_app;

#[tokio::test]
async fn migrations_seed_default_user_types_and_audit_log_table() {
    let app = spawn_app().await;

    let user_types: Vec<String> = sqlx::query_scalar("SELECT name FROM user_types ORDER BY name")
        .fetch_all(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(user_types, vec!["guest", "maintainer", "owner", "user"]);

    let admin: (String, Option<uuid::Uuid>) = sqlx::query_as(
        r#"
        SELECT user_types.name, users.laboratory_id
        FROM users
        INNER JOIN user_types USING (user_type_id)
        WHERE users.username = 'admin'
        "#,
    )
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(admin.0, "owner");
    assert!(admin.1.is_none());

    let audit_log_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs")
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(audit_log_count, 0);
}

#[tokio::test]
async fn migrations_create_inventory_foundation_tables_and_seed_units() {
    let app = spawn_app().await;

    let unit_codes: Vec<String> = sqlx::query_scalar("SELECT code FROM units ORDER BY code")
        .fetch_all(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(
        unit_codes,
        vec!["cm", "g", "kg", "l", "m", "ml", "mm", "pcs"]
    );

    let required_tables = vec![
        "asset_categories",
        "locations",
        "assets",
        "asset_inventory_items",
        "borrow_requests",
        "maintenance_records",
        "maintenance_schedules",
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
    assert_eq!(
        threshold_columns,
        vec!["minimum_stock_quantity", "minimum_stock_unit_id"]
    );

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
            'idx_asset_inventory_items_search_trgm',
            'idx_borrow_requests_purpose_trgm',
            'idx_maintenance_records_search_trgm'
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
            "idx_assets_search_trgm",
            "idx_borrow_requests_purpose_trgm",
            "idx_maintenance_records_search_trgm"
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
            asset_kind,
            tracking_mode,
            name,
            default_unit_id
        )
        VALUES ($1, $2, 'material', 'quantity', 'Resistors', $3)
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
            unit_id
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

    let result = sqlx::query(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            asset_kind,
            tracking_mode,
            name,
            default_unit_id,
            minimum_stock_quantity,
            minimum_stock_unit_id
        )
        VALUES ($1, $2, 'material', 'quantity', 'Bad Threshold', $3, -1, $3)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(result.is_err());

    let result = sqlx::query(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            asset_kind,
            tracking_mode,
            name,
            default_unit_id,
            minimum_stock_quantity
        )
        VALUES ($1, $2, 'material', 'quantity', 'Partial Threshold', $3, 1)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(laboratory_id)
    .bind(unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(result.is_err());
}
