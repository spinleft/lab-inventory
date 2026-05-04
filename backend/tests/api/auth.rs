use crate::helpers::spawn_app;
use reqwest::header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, ORIGIN};
use uuid::Uuid;

#[tokio::test]
async fn returns_401_on_login_failure() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post_login(&login_body).await;

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Authentication failed");
}

#[tokio::test]
async fn returns_200_on_login_success() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    });
    let response = app.post_login(&login_body).await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Login successful");
}

#[tokio::test]
async fn auth_me_requires_authentication() {
    let app = spawn_app().await;

    let response = app.get_me().await;

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Authentication required");
}

#[tokio::test]
async fn auth_me_unauthorized_response_includes_cors_headers() {
    let app = spawn_app().await;

    let response = app
        .api_client
        .get(format!("{}/api/v1/auth/me", &app.address))
        .header(ORIGIN, "http://127.0.0.1:5173")
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://127.0.0.1:5173")
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Authentication required");
}

#[tokio::test]
async fn auth_me_returns_current_user_after_login() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app.get_me().await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user_id"], app.test_user.user_id.to_string());
    assert_eq!(body["username"], app.test_user.username);
    assert_eq!(body["user_type"]["name"], "owner");
    assert!(body["laboratory"].is_null());
}

#[tokio::test]
async fn logout_clears_the_current_session() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    assert_eq!(app.get_me().await.status().as_u16(), 200);

    let response = app.post_logout().await;
    assert_eq!(response.status().as_u16(), 200);

    let response = app.get_me().await;
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn change_password_requires_authentication() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .patch_auth_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Authentication required");
}

#[tokio::test]
async fn current_password_must_be_correct_to_change_password() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .patch_auth_password(&serde_json::json!({
            "current_password": Uuid::new_v4().to_string(),
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Current password is incorrect");

    let response = app.post_logout().await;
    assert_eq!(response.status().as_u16(), 200);
    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn new_password_confirmation_must_match() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app
        .patch_auth_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": Uuid::new_v4().to_string(),
            "new_password_check": Uuid::new_v4().to_string(),
        }))
        .await;

    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "New password confirmation does not match");

    let response = app.post_logout().await;
    assert_eq!(response.status().as_u16(), 200);
    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn changing_password_keeps_session_valid_and_allows_new_password_login() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .patch_auth_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Password changed");
    assert_eq!(app.get_me().await.status().as_u16(), 200);

    let response = app.post_logout().await;
    assert_eq!(response.status().as_u16(), 200);
    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;
    assert_eq!(response.status().as_u16(), 401);
    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &new_password,
        }))
        .await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn changing_password_records_a_non_sensitive_audit_log() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .patch_auth_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(response.status().as_u16(), 200);
    let details: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT details
        FROM audit_logs
        WHERE actor_user_id = $1
          AND target_laboratory_id IS NULL
          AND action = 'update'
          AND resource_type = 'user'
          AND resource_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(app.test_user.user_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        details,
        serde_json::json!({
            "username": app.test_user.username,
            "changed_fields": ["password"],
        })
    );
    let details = details.to_string();
    assert!(!details.contains(&app.test_user.password));
    assert!(!details.contains(&new_password));
    assert!(!details.contains("password_hash"));
}
