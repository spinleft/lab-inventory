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
    assert_eq!(response.status().as_u16(), 403);
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
    assert_eq!(response.status().as_u16(), 403);

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
async fn update_asset_parameter_replaces_options_and_delete_rejects_referenced_parameters() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Asset Parameter Update Lab").await;
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
    assert!(!option_is_archived(&updated, "gas"));
    assert!(option_is_archived(&updated, "liquid"));

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

    insert_assignment(&app, laboratory_id, parameter_id, category_id(&category)).await;
    let response = app.delete_asset_parameter(parameter_id).await;
    assert_eq!(response.status().as_u16(), 409);

    sqlx::query("DELETE FROM asset_parameter_assignments WHERE parameter_type_id = $1")
        .bind(parameter_id)
        .execute(&app.db_pool)
        .await
        .unwrap();
    let response = app.delete_asset_parameter(parameter_id).await;
    assert_eq!(response.status().as_u16(), 204);
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

fn option_is_archived(parameter: &serde_json::Value, code: &str) -> bool {
    parameter["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["code"] == code)
        .unwrap()["is_archived"]
        .as_bool()
        .unwrap()
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
