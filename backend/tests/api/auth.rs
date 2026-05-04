use crate::helpers::spawn_app;

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
async fn auth_me_returns_current_user_after_login() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;

    let response = app.get_me().await;

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user_id"], app.test_user.user_id.to_string());
    assert_eq!(body["username"], app.test_user.username);
    assert_eq!(body["group"]["name"], "system_admin");
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
