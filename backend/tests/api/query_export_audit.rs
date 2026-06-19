use crate::helpers::{TestUser, spawn_app};

#[tokio::test]
async fn audit_logs_are_permissioned_and_filterable() {
    let app = spawn_app().await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 401);

    let laboratory_id = app.create_laboratory("Audit Forbidden Lab").await;
    let root = TestUser::generate_with_user_type("root", None);
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    let user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&root).await;
    app.store_user(&lab_admin).await;
    app.store_user(&user).await;

    root.login(&app).await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 200);

    lab_admin.login(&app).await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 403);

    user.login(&app).await;
    let response = app.get_api_path("/audit-logs").await;
    assert_eq!(response.status().as_u16(), 403);

    app.test_user.login(&app).await;
    let first_response = app
        .post_laboratory(&serde_json::json!({
            "name": "Audit Page Lab A",
            "address": "Building A"
        }))
        .await;
    assert_eq!(first_response.status().as_u16(), 201);
    let first_laboratory: serde_json::Value = first_response.json().await.unwrap();
    let first_laboratory_id = first_laboratory["laboratory_id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .unwrap();
    let second_response = app
        .post_laboratory(&serde_json::json!({
            "name": "Audit Page Lab B",
            "address": "Building B"
        }))
        .await;
    assert_eq!(second_response.status().as_u16(), 201);

    let response = app
        .get_api_path("/audit-logs?resource_type=laboratory&action=create&limit=1&offset=0")
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let logs: serde_json::Value = response.json().await.unwrap();
    assert_eq!(logs["limit"], 1);
    assert_eq!(logs["offset"], 0);
    assert!(logs["total"].as_i64().unwrap() >= 2);
    assert_eq!(logs["items"].as_array().unwrap().len(), 1);
    assert_eq!(logs["items"][0]["resource_type"], "laboratory");
    assert_eq!(logs["items"][0]["action"], "create");

    let response = app
        .get_api_path(&format!(
            "/audit-logs?resource_type=laboratory&resource_id={first_laboratory_id}&action=create"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let filtered_logs: serde_json::Value = response.json().await.unwrap();
    assert_eq!(filtered_logs["total"], 1);
    assert_eq!(
        filtered_logs["items"][0]["resource_id"],
        first_laboratory_id.to_string()
    );
    assert_eq!(
        filtered_logs["items"][0]["actor_user_id"],
        app.test_user.user_id.to_string()
    );
}
