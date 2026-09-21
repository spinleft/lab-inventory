use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn create_list_get_asset_parameters_are_laboratory_scoped_and_record_audit() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Asset Parameter Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Asset Parameter Other Lab").await;
    app.test_user.login(&app).await;

    let own_parameter = create_enum_parameter(&app, own_laboratory_id, "state").await;
    let own_parameter_id = parameter_id(&own_parameter);
    let other_parameter = create_enum_parameter(&app, other_laboratory_id, "state").await;

    assert_eq!(
        own_parameter["laboratory_id"],
        own_laboratory_id.to_string()
    );
    assert_eq!(own_parameter["code"], "state");
    assert_eq!(own_parameter["name"], "State");
    assert_eq!(own_parameter["data_type"], "enum");
    assert_eq!(own_parameter["options"].as_array().unwrap().len(), 2);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        own_parameter_id,
        "create",
        "asset_parameter",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "delete");
    assert_eq!(
        audit_details["rollback"]["resource_type"],
        "asset_parameter"
    );

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    let response = app.get_asset_parameters(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(parameter_codes(&body), vec!["state"]);

    let response = app.get_asset_parameter(own_parameter_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["parameter_type_id"], own_parameter_id.to_string());

    let response = app.get_asset_parameters(other_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(parameter_codes(&body), vec!["state"]);

    let response = app
        .get_asset_parameter(parameter_id(&other_parameter))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn laboratory_users_can_manage_own_asset_parameters_but_guests_and_cross_lab_users_cannot() {
    let app = spawn_app().await;
    let own_laboratory_id = app
        .create_laboratory("Asset Parameter Permission Own Lab")
        .await;
    let other_laboratory_id = app
        .create_laboratory("Asset Parameter Permission Other Lab")
        .await;

    let response = app
        .post_asset_parameter(
            own_laboratory_id,
            &serde_json::json!({
                "code": "unauthenticated",
                "name": "Unauthenticated",
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 401);

    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;
    let response = app
        .post_asset_parameter(
            own_laboratory_id,
            &serde_json::json!({
                "code": "guest_parameter",
                "name": "Guest Parameter",
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;
    let response = app
        .post_asset_parameter(
            own_laboratory_id,
            &serde_json::json!({
                "code": "managed_by_user",
                "name": "Managed By User",
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let parameter: serde_json::Value = response.json().await.unwrap();
    let parameter_id = parameter_id(&parameter);

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "name": "Updated By User",
                "description": "Updated from test"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app
        .post_asset_parameter(
            other_laboratory_id,
            &serde_json::json!({
                "code": "cross_lab",
                "name": "Cross Lab",
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);

    let response = app.delete_asset_parameter(parameter_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn create_asset_parameter_rejects_invalid_conflicting_or_inconsistent_input() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Validation Lab")
        .await;

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "InvalidCode",
                "name": "Invalid Code",
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "text_with_unit",
                "name": "Text With Unit",
                "data_type": "text",
                "unit_dimension": "length"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "enum_without_options",
                "name": "Enum Without Options",
                "data_type": "enum"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let millimeter_unit_id = app.unit_id("mm").await;
    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "missing_default_unit",
                "name": "Missing Default Unit",
                "data_type": "number",
                "default_unit_id": Uuid::new_v4()
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "missing_range_default_unit",
                "name": "Missing Range Default Unit",
                "data_type": "range",
                "default_unit_id": Uuid::new_v4()
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "bad_default_unit",
                "name": "Bad Default Unit",
                "data_type": "number",
                "unit_dimension": "mass",
                "default_unit_id": millimeter_unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "length_number",
                "name": "Length Number",
                "data_type": "number",
                "default_unit_id": millimeter_unit_id
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let parameter: serde_json::Value = response.json().await.unwrap();
    assert_eq!(parameter["unit_dimension"], "length");

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "length_number",
                "name": "Duplicate Length Number",
                "data_type": "number"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 409);
}

#[tokio::test]
async fn range_asset_parameters_allow_units_and_enforce_value_bounds() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Parameter Range Lab").await;
    let nanometer_unit_id = insert_nanometer_unit(&app, laboratory_id).await;

    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": "wavelength_range",
                "name": "Wavelength Range",
                "data_type": "range",
                "default_unit_id": nanometer_unit_id,
                "description": "Visible spectrum, for example 400-700 nm"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let parameter: serde_json::Value = response.json().await.unwrap();
    let parameter_id = parameter_id(&parameter);
    assert_eq!(parameter["data_type"], "range");
    assert_eq!(parameter["unit_dimension"], "length");
    assert_eq!(parameter["default_unit_id"], nanometer_unit_id.to_string());

    let asset_id = insert_test_asset(&app, laboratory_id, nanometer_unit_id).await;
    sqlx::query(
        r#"
        INSERT INTO asset_parameter_values (
            value_id,
            laboratory_id,
            asset_id,
            parameter_type_id,
            data_type,
            value_range_start,
            value_range_end,
            value_range_start_in_base,
            value_range_end_in_base,
            unit_id
        )
        VALUES ($1, $2, $3, $4, 'range', 400, 700, 0.0000004, 0.0000007, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(asset_id)
    .bind(parameter_id)
    .bind(nanometer_unit_id)
    .execute(&app.db_pool)
    .await
    .unwrap();

    let invalid_asset_id = insert_test_asset(&app, laboratory_id, nanometer_unit_id).await;
    let result = sqlx::query(
        r#"
        INSERT INTO asset_parameter_values (
            value_id,
            laboratory_id,
            asset_id,
            parameter_type_id,
            data_type,
            value_range_start,
            value_range_end,
            unit_id
        )
        VALUES ($1, $2, $3, $4, 'range', 700, 400, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(invalid_asset_id)
    .bind(parameter_id)
    .bind(nanometer_unit_id)
    .execute(&app.db_pool)
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_asset_parameter_replaces_options_and_delete_cascades_assignments_and_values() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Parameter Update Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_enum_parameter(&app, laboratory_id, "color").await;
    let parameter_id = parameter_id(&parameter);
    let solid_option_id = option_id(&parameter, "solid");
    let category = create_category(&app, laboratory_id, "Equipment", "equipment").await;

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "default_unit_id": Uuid::new_v4()
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "code": "material_state",
                "name": "Material State",
                "options": [
                    {
                        "option_id": solid_option_id,
                        "code": "solid",
                        "label": "Solid material",
                        "sort_order": 2
                    },
                    {
                        "code": "gas",
                        "label": "Gas",
                        "sort_order": 1
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["code"], "material_state");
    assert_eq!(option_label(&updated, "solid"), "Solid material");
    assert!(option_exists(&updated, "gas"));
    assert!(!option_exists(&updated, "liquid"));

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        parameter_id,
        "update",
        "asset_parameter",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "update");
    assert_eq!(audit_details["rollback"]["values"]["code"], "color");
    assert!(
        audit_details["rollback"]["values"]["parameter_values"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    insert_assignment(&app, laboratory_id, parameter_id, category_id(&category)).await;
    let (_, value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "option_id": solid_option_id.to_string() }),
    )
    .await;

    let response = app.delete_asset_parameter(parameter_id).await;
    assert_eq!(response.status().as_u16(), 204);

    let assignment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM asset_parameter_assignments WHERE parameter_type_id = $1",
    )
    .bind(parameter_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(assignment_count, 0);
    let value_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asset_parameter_values WHERE value_id = $1")
            .bind(value_id)
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_eq!(value_count, 0);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        parameter_id,
        "delete",
        "asset_parameter",
    )
    .await;
    assert_eq!(audit_details["rollback"]["operation"], "create");
    assert_eq!(
        audit_details["rollback"]["values"]["assignments"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        audit_details["rollback"]["values"]["parameter_values"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn changing_the_data_type_requires_a_migration_for_every_stored_value() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Parameter Migration Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_text_parameter(&app, laboratory_id, "serial_no").await;
    let parameter_id = parameter_id(&parameter);
    create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "AA-1" }),
    )
    .await;
    create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "AA-2" }),
    )
    .await;
    let stored_value_ids = stored_value_ids(&app, parameter_id).await;
    assert_eq!(stored_value_ids.len(), 2);
    let first_value_id = stored_value_ids[0];
    let second_value_id = stored_value_ids[1];

    // No migration at all.
    let response = app
        .patch_asset_parameter(parameter_id, &serde_json::json!({ "data_type": "number" }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // Only one of the two stored values.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "value_migrations": [
                    { "value_id": first_value_id, "value": { "number": 1.0 } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // A value the parameter does not store.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "value_migrations": [
                    { "value_id": first_value_id, "value": { "number": 1.0 } },
                    { "value_id": second_value_id, "value": { "number": 2.0 } },
                    { "value_id": Uuid::new_v4(), "value": { "number": 3.0 } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // The same value twice.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "value_migrations": [
                    { "value_id": first_value_id, "value": { "number": 1.0 } },
                    { "value_id": first_value_id, "value": { "number": 2.0 } },
                    { "value_id": second_value_id, "value": { "number": 3.0 } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // A value the new type cannot read.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "value_migrations": [
                    { "value_id": first_value_id, "value": { "text": "AA-1" } },
                    { "value_id": second_value_id, "value": { "number": 2.0 } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // Migrations without a data type change.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "name": "Serial Number",
                "value_migrations": [
                    { "value_id": first_value_id, "value": { "text": "AA-1" } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // Every rejected request left the parameter and its values alone.
    let response = app.get_asset_parameter(parameter_id).await;
    let parameter: serde_json::Value = response.json().await.unwrap();
    assert_eq!(parameter["data_type"], "text");
    assert_eq!(parameter["name"], "Serial No");
    for value_id in [first_value_id, second_value_id] {
        let stored = stored_value(&app, value_id).await;
        assert_eq!(stored.data_type, "text");
        assert!(
            stored
                .value_text
                .as_deref()
                .is_some_and(|text| text.starts_with("AA-")),
            "stored value changed to {:?}",
            stored.value_text
        );
    }
}

#[tokio::test]
async fn changing_the_data_type_rewrites_values_in_place_and_keeps_the_old_columns() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Value Migration Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    let nanometer_unit_id = insert_nanometer_unit(&app, laboratory_id).await;
    let parameter = create_text_parameter(&app, laboratory_id, "thickness").await;
    let parameter_id = parameter_id(&parameter);
    let (asset_id, value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "2.5" }),
    )
    .await;
    let (other_asset_id, other_value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "7" }),
    )
    .await;

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "number",
                "default_unit_id": nanometer_unit_id,
                "value_migrations": [
                    {
                        "value_id": value_id,
                        "value": { "number": 2.5, "unit_id": nanometer_unit_id }
                    },
                    {
                        "value_id": other_value_id,
                        "value": { "number": 7.0, "unit_id": nanometer_unit_id }
                    }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "number");
    assert_eq!(updated["unit_dimension"], "length");

    // The row keeps its identity and its old columns, and gains the new ones.
    let stored = stored_value(&app, value_id).await;
    assert_eq!(stored.value_id, value_id);
    assert_eq!(stored.data_type, "number");
    assert_eq!(stored.value_text.as_deref(), Some("2.5"));
    assert_eq!(stored.value_number, Some(2.5));
    assert_eq!(stored.unit_id, Some(nanometer_unit_id));
    let value_in_base = stored.value_number_in_base.unwrap();
    assert!((value_in_base - 0.0000000025).abs() < 1e-18);

    let response = app
        .get_asset_with_query(asset_id, "include=parameters")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let asset: serde_json::Value = response.json().await.unwrap();
    assert_eq!(asset["parameters"][0]["data_type"], "number");
    assert_eq!(asset["parameters"][0]["value"]["number"], 2.5);
    assert_eq!(
        asset["parameters"][0]["value"]["unit_id"],
        nanometer_unit_id.to_string()
    );

    let response = app
        .get_asset_with_query(other_asset_id, "include=parameters")
        .await;
    let other_asset: serde_json::Value = response.json().await.unwrap();
    assert_eq!(other_asset["parameters"][0]["value"]["number"], 7.0);

    let audit_details = latest_audit_details(
        &app,
        app.test_user.user_id,
        parameter_id,
        "update",
        "asset_parameter",
    )
    .await;
    let pre_migration_values = audit_details["rollback"]["values"]["parameter_values"]
        .as_array()
        .unwrap();
    assert_eq!(pre_migration_values.len(), 2);
    assert!(
        pre_migration_values
            .iter()
            .any(|value| value["value_text"] == "2.5" && value["data_type"] == "text")
    );
}

#[tokio::test]
async fn changing_the_data_type_away_from_enum_moves_values_off_the_options() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Enum Migration Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_enum_parameter(&app, laboratory_id, "state").await;
    let parameter_id = parameter_id(&parameter);
    let solid_option_id = option_id(&parameter, "solid");
    let (_, value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "option_id": solid_option_id.to_string() }),
    )
    .await;

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "text",
                "value_migrations": [
                    { "value_id": value_id, "value": { "text": "solid" } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "text");
    assert!(updated["options"].as_array().unwrap().is_empty());

    let stored = stored_value(&app, value_id).await;
    assert_eq!(stored.data_type, "text");
    assert_eq!(stored.value_text.as_deref(), Some("solid"));
    // The options are gone with the enum type, and a leftover option id would
    // break the value-to-option foreign key.
    assert_eq!(stored.value_option_id, None);
    let option_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM asset_parameter_options WHERE parameter_type_id = $1",
    )
    .bind(parameter_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(option_count, 0);
}

#[tokio::test]
async fn changing_the_data_type_to_enum_points_values_at_the_new_options() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Enum Target Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_text_parameter(&app, laboratory_id, "material_state").await;
    let parameter_id = parameter_id(&parameter);
    let (asset_id, value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "solid" }),
    )
    .await;

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "enum",
                "options": [
                    { "code": "solid", "label": "Solid", "sort_order": 1 },
                    { "code": "liquid", "label": "Liquid", "sort_order": 2 }
                ],
                "value_migrations": [
                    { "value_id": value_id, "value": { "option_code": "solid" } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "enum");
    let solid_option_id = option_id(&updated, "solid");

    let stored = stored_value(&app, value_id).await;
    assert_eq!(stored.value_option_id, Some(solid_option_id));
    // The text the value had before the change stays in the same row.
    assert_eq!(stored.value_text.as_deref(), Some("solid"));

    let response = app
        .get_asset_with_query(asset_id, "include=parameters")
        .await;
    let asset: serde_json::Value = response.json().await.unwrap();
    assert_eq!(asset["parameters"][0]["data_type"], "enum");
    assert_eq!(asset["parameters"][0]["value"]["option_code"], "solid");
}

#[tokio::test]
async fn changing_the_data_type_to_enum_creates_the_options_the_values_migrate_onto() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Enum Creation Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_text_parameter(&app, laboratory_id, "material_state").await;
    let parameter_id = parameter_id(&parameter);
    let (_, solid_value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "solid" }),
    )
    .await;
    let (_, gas_value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "gas" }),
    )
    .await;

    // An enum still needs options from somewhere: neither the parameter nor the
    // migrations bring any.
    let response = app
        .patch_asset_parameter(parameter_id, &serde_json::json!({ "data_type": "enum" }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // A code that could never be an option code is rejected, and nothing lands.
    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "enum",
                "value_migrations": [
                    { "value_id": solid_value_id, "value": { "option_code": "Solid!" } },
                    { "value_id": gas_value_id, "value": { "option_code": "gas" } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "enum",
                "value_migrations": [
                    {
                        "value_id": solid_value_id,
                        "value": { "option_code": "solid", "option_label": "Solid" }
                    },
                    { "value_id": gas_value_id, "value": { "option_code": "gas" } }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "enum");
    // The migrations named both options, so the parameter has both of them.
    assert_eq!(updated["options"].as_array().unwrap().len(), 2);
    assert_eq!(option_label(&updated, "solid"), "Solid");
    assert_eq!(option_label(&updated, "gas"), "gas");

    let stored = stored_value(&app, solid_value_id).await;
    assert_eq!(stored.value_option_id, Some(option_id(&updated, "solid")));
    assert_eq!(stored.value_text.as_deref(), Some("solid"));
    let stored = stored_value(&app, gas_value_id).await;
    assert_eq!(stored.value_option_id, Some(option_id(&updated, "gas")));
}

#[tokio::test]
async fn changing_the_data_type_can_drop_a_stored_value() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Drop Value Lab")
        .await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_text_parameter(&app, laboratory_id, "note").await;
    let parameter_id = parameter_id(&parameter);
    let (_, value_id) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        parameter_id,
        &serde_json::json!({ "text": "keep me" }),
    )
    .await;

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({
                "data_type": "boolean",
                "value_migrations": [{ "value_id": value_id, "value": null }]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "boolean");
    assert!(stored_value_ids(&app, parameter_id).await.is_empty());
}

#[tokio::test]
async fn changing_the_data_type_without_stored_values_needs_no_migration() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app
        .create_laboratory("Asset Parameter Empty Migration Lab")
        .await;
    let parameter = create_text_parameter(&app, laboratory_id, "note").await;
    let parameter_id = parameter_id(&parameter);

    let response = app
        .patch_asset_parameter(parameter_id, &serde_json::json!({ "data_type": "date" }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["data_type"], "date");

    let response = app
        .patch_asset_parameter(
            parameter_id,
            &serde_json::json!({ "data_type": "date", "value_migrations": [] }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn listing_asset_parameter_values_covers_the_parameters_stored_values() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Parameter Values Lab").await;
    let unit_id = app.unit_id("pcs").await;
    let parameter = create_text_parameter(&app, laboratory_id, "serial_no").await;
    let serial_no_parameter_id = parameter_id(&parameter);
    let (asset_id, _) = create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        serial_no_parameter_id,
        &serde_json::json!({ "text": "AA-1" }),
    )
    .await;
    create_asset_with_parameter_value(
        &app,
        laboratory_id,
        unit_id,
        serial_no_parameter_id,
        &serde_json::json!({ "text": "AA-2" }),
    )
    .await;
    let empty_parameter = create_text_parameter(&app, laboratory_id, "note").await;

    let response = app.get_asset_parameter_values(serial_no_parameter_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let values: serde_json::Value = response.json().await.unwrap();
    let values = values.as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["data_type"], "text");
    assert_eq!(values[0]["asset_model"], serde_json::Value::Null);
    assert!(
        values
            .iter()
            .any(|value| value["asset_id"] == asset_id.to_string()
                && value["value"]["text"] == "AA-1")
    );
    assert!(values.iter().any(|value| value["value"]["text"] == "AA-2"));

    let response = app
        .get_asset_parameter_values(parameter_id(&empty_parameter))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let values: serde_json::Value = response.json().await.unwrap();
    assert!(values.as_array().unwrap().is_empty());

    let response = app.get_asset_parameter_values(Uuid::new_v4()).await;
    assert_eq!(response.status().as_u16(), 403);

    let regular_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;
    let response = app.get_asset_parameter_values(serial_no_parameter_id).await;
    assert_eq!(response.status().as_u16(), 200);
}

async fn create_enum_parameter(
    app: &TestApp,
    laboratory_id: Uuid,
    code: &str,
) -> serde_json::Value {
    let response = app
        .post_asset_parameter(
            laboratory_id,
            &serde_json::json!({
                "code": code,
                "name": title_case_code(code),
                "data_type": "enum",
                "options": [
                    { "code": "solid", "label": "Solid", "sort_order": 1 },
                    { "code": "liquid", "label": "Liquid", "sort_order": 2 }
                ]
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

async fn create_category(
    app: &TestApp,
    laboratory_id: Uuid,
    name: &str,
    code: &str,
) -> serde_json::Value {
    let response = app
        .post_asset_category(
            laboratory_id,
            &serde_json::json!({
                "name": name,
                "code": code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
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
                "name": title_case_code(code),
                "data_type": "text"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    response.json().await.unwrap()
}

/// Records one value for the parameter on a new asset through the API, and
/// returns the asset id together with the stored value id.
async fn create_asset_with_parameter_value(
    app: &TestApp,
    laboratory_id: Uuid,
    unit_id: Uuid,
    parameter_id: Uuid,
    value: &serde_json::Value,
) -> (Uuid, Uuid) {
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "inventory_unit_id": unit_id,
                "name": format!("Parameterized Asset {}", Uuid::new_v4()),
                "parameters": [{ "parameter_type_id": parameter_id, "value": value }],
                "tracking_mode": "quantity"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let asset: serde_json::Value = response.json().await.unwrap();
    let asset_id = asset["asset_id"].as_str().unwrap().parse().unwrap();
    let value_id = sqlx::query_scalar(
        r#"
        SELECT value_id
        FROM asset_parameter_values
        WHERE asset_id = $1
          AND parameter_type_id = $2
        "#,
    )
    .bind(asset_id)
    .bind(parameter_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();

    (asset_id, value_id)
}

/// A stored value as it sits on disk, including the columns a data type change
/// leaves behind.
#[derive(sqlx::FromRow)]
struct StoredValue {
    value_id: Uuid,
    data_type: String,
    value_text: Option<String>,
    value_number: Option<f64>,
    value_number_in_base: Option<f64>,
    unit_id: Option<Uuid>,
    value_option_id: Option<Uuid>,
}

async fn stored_value(app: &TestApp, value_id: Uuid) -> StoredValue {
    sqlx::query_as(
        r#"
        SELECT
            value_id,
            data_type::text AS data_type,
            value_text,
            value_number,
            value_number_in_base,
            unit_id,
            value_option_id
        FROM asset_parameter_values
        WHERE value_id = $1
        "#,
    )
    .bind(value_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

async fn stored_value_ids(app: &TestApp, parameter_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        r#"
        SELECT value_id
        FROM asset_parameter_values
        WHERE parameter_type_id = $1
        ORDER BY value_id
        "#,
    )
    .bind(parameter_id)
    .fetch_all(&app.db_pool)
    .await
    .unwrap()
}

async fn insert_assignment(
    app: &TestApp,
    laboratory_id: Uuid,
    parameter_id: Uuid,
    category_id: Uuid,
) {
    sqlx::query(
        r#"
        INSERT INTO asset_parameter_assignments (
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id
        )
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(parameter_id)
    .bind(category_id)
    .execute(&app.db_pool)
    .await
    .unwrap();
}

async fn insert_nanometer_unit(app: &TestApp, laboratory_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO units (
            unit_id,
            laboratory_id,
            code,
            name,
            symbol,
            dimension,
            scale_to_base,
            allow_decimal
        )
        VALUES ($1, $2, 'nm', 'Nanometer', 'nm', 'length', 0.000000001, true)
        RETURNING unit_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

async fn insert_test_asset(app: &TestApp, laboratory_id: Uuid, unit_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            tracking_mode,
            name,
            inventory_unit_id
        )
        VALUES ($1, $2, 'quantity', $3, $4)
        RETURNING asset_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(format!("Test Asset {}", Uuid::new_v4()))
    .bind(unit_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}

fn parameter_id(parameter: &serde_json::Value) -> Uuid {
    parameter["parameter_type_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn category_id(category: &serde_json::Value) -> Uuid {
    category["category_id"].as_str().unwrap().parse().unwrap()
}

fn option_id(parameter: &serde_json::Value, code: &str) -> Uuid {
    parameter["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["code"] == code)
        .unwrap()["option_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn option_label<'a>(parameter: &'a serde_json::Value, code: &str) -> &'a str {
    parameter["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["code"] == code)
        .unwrap()["label"]
        .as_str()
        .unwrap()
}

fn option_exists(parameter: &serde_json::Value, code: &str) -> bool {
    parameter["options"]
        .as_array()
        .unwrap()
        .iter()
        .any(|option| option["code"] == code)
}

fn parameter_codes(body: &serde_json::Value) -> Vec<&str> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["code"].as_str().unwrap())
        .collect()
}

fn title_case_code(code: &str) -> String {
    code.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn latest_audit_details(
    app: &TestApp,
    actor_user_id: Uuid,
    resource_id: Uuid,
    action: &str,
    resource_type: &str,
) -> serde_json::Value {
    sqlx::query_scalar(
        r#"
        SELECT details
        FROM audit_logs
        WHERE actor_user_id = $1
          AND action = $2
          AND resource_type = $3
          AND resource_id = $4
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap()
}
