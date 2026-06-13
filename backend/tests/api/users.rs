use crate::helpers::{TestUser, spawn_app};

#[tokio::test]
async fn owner_can_create_users_in_any_laboratory() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Physics Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "physics-admin",
            "password": "password",
            "user_type": "maintainer",
            "laboratory_id": laboratory_id,
            "email": "physics-admin@example.com"
        }))
        .await;

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "physics-admin");
    assert_eq!(body["user_type"]["name"], "maintainer");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );
}

#[tokio::test]
async fn owner_can_list_update_and_delete_users() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Owner Managed Lab").await;
    let target = TestUser::generate_with_user_type("maintainer", Some(laboratory_id));
    app.store_user(&target).await;

    let response = app.get_api_path("/users").await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|user| user["user_id"] == target.user_id.to_string())
    );

    let response = app
        .patch_user(
            target.user_id,
            &serde_json::json!({
                "username": "root-managed-user",
                "password": "new-password",
                "user_type": "owner",
                "laboratory_id": null,
                "email": null
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "root-managed-user");
    assert_eq!(body["user_type"]["name"], "owner");
    assert!(body["laboratory"].is_null());
    assert!(body["email"].is_null());

    let response = app.delete_user(target.user_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn creating_a_user_records_an_audit_log() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Audit User Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "audited-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    let user_id: uuid::Uuid = body["user_id"].as_str().unwrap().parse().unwrap();

    let audit_log_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_logs
        WHERE actor_user_id = $1
          AND target_laboratory_id = $2
          AND action = 'create'
          AND resource_type = 'user'
          AND resource_id = $3
          AND details->>'username' = 'audited-user'
        "#,
    )
    .bind(app.test_user.user_id)
    .bind(laboratory_id)
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(audit_log_count, 1);
}

#[tokio::test]
async fn maintainer_can_list_and_manage_own_lab_maintainer_user_and_guest() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Materials Lab").await;
    let other_laboratory_id = app.create_laboratory("Other Materials Lab").await;
    let maintainer = TestUser::generate_with_user_type("maintainer", Some(laboratory_id));
    let own_lab_user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    let other_lab_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&maintainer).await;
    app.store_user(&own_lab_user).await;
    app.store_user(&other_lab_user).await;
    maintainer.login(&app).await;

    let response = app.get_api_path("/users").await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|user| user["user_id"] == maintainer.user_id.to_string())
    );
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .all(|user| user["laboratory"]["laboratory_id"] == laboratory_id.to_string())
    );

    let response = app
        .patch_user(
            own_lab_user.user_id,
            &serde_json::json!({ "user_type": "maintainer" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user_type"]["name"], "maintainer");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );

    let response = app
        .post_user(&serde_json::json!({
            "username": "materials-maintainer",
            "password": "password",
            "user_type": "maintainer"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user_type"]["name"], "maintainer");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );

    let user_id = body["user_id"].as_str().unwrap().parse().unwrap();
    let response = app
        .patch_user(
            user_id,
            &serde_json::json!({
                "username": "materials-guest",
                "email": "updated@example.com",
                "password": "new-password",
                "user_type": "guest"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["username"], "materials-guest");
    assert_eq!(body["user_type"]["name"], "guest");
    assert_eq!(
        body["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );

    let response = app.delete_user(user_id).await;
    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn maintainer_cannot_create_owner_or_manage_other_laboratory_users() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Other Lab").await;
    let maintainer = TestUser::generate_with_user_type("maintainer", Some(own_laboratory_id));
    let other_user = TestUser::generate_with_user_type("user", Some(other_laboratory_id));
    app.store_user(&maintainer).await;
    app.store_user(&other_user).await;
    maintainer.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "bad-admin",
            "password": "password",
            "user_type": "owner"
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .post_user(&serde_json::json!({
            "username": "other-lab-user",
            "password": "password",
            "user_type": "user",
            "laboratory_id": other_laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app.delete_user(other_user.user_id).await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .patch_user(
            other_user.user_id,
            &serde_json::json!({ "email": "bad-update@example.com" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let response = app
        .patch_user(
            app.test_user.user_id,
            &serde_json::json!({ "email": "owner-update@example.com" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn maintainer_cannot_delete_or_rescope_self() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Self Scope Lab").await;
    let maintainer = TestUser::generate_with_user_type("maintainer", Some(laboratory_id));
    app.store_user(&maintainer).await;
    maintainer.login(&app).await;

    let response = app.delete_user(maintainer.user_id).await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_user(
            maintainer.user_id,
            &serde_json::json!({ "user_type": "user" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let response = app
        .patch_user(
            maintainer.user_id,
            &serde_json::json!({ "email": "self-update@example.com" }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn user_create_and_password_reset_reject_blank_passwords() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Password Lab").await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "blank-password",
            "password": "   ",
            "user_type": "user",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let target = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&target).await;

    let response = app
        .patch_user(target.user_id, &serde_json::json!({ "password": "" }))
        .await;
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn regular_user_cannot_manage_users() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Biology Lab").await;
    let user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&user).await;
    user.login(&app).await;

    let response = app
        .post_user(&serde_json::json!({
            "username": "biology-guest",
            "password": "password",
            "user_type": "guest",
            "laboratory_id": laboratory_id
        }))
        .await;
    assert_eq!(response.status().as_u16(), 403);
}
