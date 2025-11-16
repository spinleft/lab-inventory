/*
 * @Author: spinleft spinleftgit@gmail.com
 * @Date: 2025-10-19 23:01:50
 * @LastEditors: spinleft spinleftgit@gmail.com
 * @LastEditTime: 2025-10-20 01:17:57
 * @FilePath: \lab-inventory\backend\tests\api\auth.rs
 * @Description:
 *
 * Copyright (c) 2025 by ${git_name_email}, All Rights Reserved.
 */
use crate::helpers::spawn_app;

#[tokio::test]
async fn returns_401_on_failure() {
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Try to login
    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post_login(&login_body).await;

    // Assert
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn returns_200_on_login_success() {
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Login
    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    });
    let response = app.post_login(&login_body).await;

    // Assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn returns_200_on_logout() {
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Login
    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    });
    let response = app.post_login(&login_body).await;
    assert_eq!(response.status().as_u16(), 200);

    // Act - Part 2 - Logout
    let response = app.post_logout().await;
    assert_eq!(response.status().as_u16(), 200);
}
