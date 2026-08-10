use crate::helpers::{TestApp, TestUser, spawn_app};
use chrono::{DateTime, Duration, Utc};
use reqwest::StatusCode;
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

async fn issue_code(app: &TestApp, issuer: &TestUser, laboratory_id: Uuid) -> serde_json::Value {
    issuer.login(app).await;
    let response = app.post_guest_registration_code(laboratory_id).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

fn registration_body(
    username: &str,
    email: &str,
    phone_number: &str,
    registration_code: &str,
) -> serde_json::Value {
    serde_json::json!({
        "username": username,
        "password": "password",
        "email": email,
        "phone_number": phone_number,
        "registration_code": registration_code,
    })
}

#[tokio::test]
async fn only_lab_admins_and_users_can_issue_codes_for_their_laboratory() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Code Permission Lab").await;
    let other_laboratory_id = app.create_laboratory("Guest Code Other Lab").await;

    assert_eq!(
        app.post_guest_registration_code(laboratory_id)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );

    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    let user = TestUser::generate_with_user_type("user", Some(laboratory_id));
    let guest = TestUser::generate_with_user_type("guest", Some(laboratory_id));
    for actor in [&lab_admin, &user, &guest] {
        app.store_user(actor).await;
    }

    assert_eq!(
        issue_code(&app, &lab_admin, laboratory_id).await["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(
        issue_code(&app, &user, laboratory_id).await["laboratory_id"],
        laboratory_id.to_string()
    );

    user.login(&app).await;
    assert_eq!(
        app.post_guest_registration_code(other_laboratory_id)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    guest.login(&app).await;
    assert_eq!(
        app.post_guest_registration_code(laboratory_id)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    app.test_user.login(&app).await;
    assert_eq!(
        app.post_guest_registration_code(laboratory_id)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn issued_code_is_six_digits_ten_minutes_long_and_never_stored_in_plaintext() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Code Security Lab").await;
    let issuer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&issuer).await;

    let before = Utc::now();
    let body = issue_code(&app, &issuer, laboratory_id).await;
    let after = Utc::now();
    let registration_code = body["registration_code"].as_str().unwrap();
    assert_eq!(registration_code.len(), 6);
    assert!(
        registration_code
            .chars()
            .all(|character| character.is_ascii_digit())
    );
    let expires_at: DateTime<Utc> = body["expires_at"].as_str().unwrap().parse().unwrap();
    assert!(expires_at >= before + Duration::minutes(10));
    assert!(expires_at <= after + Duration::minutes(10));

    let registration_code_id: Uuid = body["registration_code_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let code_hmac: String = sqlx::query_scalar(
        "SELECT code_hmac FROM guest_registration_codes WHERE registration_code_id = $1",
    )
    .bind(registration_code_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_ne!(code_hmac, registration_code);
    assert_eq!(code_hmac.len(), 64);

    let audit_details: String = sqlx::query_scalar(
        r#"
        SELECT details::text
        FROM audit_logs
        WHERE resource_type = 'guest_registration_code'
          AND resource_id = $1
        "#,
    )
    .bind(registration_code_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert!(!audit_details.contains(registration_code));
    assert!(!audit_details.contains(&code_hmac));
}

#[tokio::test]
async fn registration_creates_a_guest_without_logging_it_in() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Registration Lab").await;
    let issuer = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&issuer).await;
    let code = issue_code(&app, &issuer, laboratory_id).await;
    assert_eq!(app.post_logout().await.status(), StatusCode::OK);

    let registration_code = code["registration_code"].as_str().unwrap();
    let response = app
        .post_guest_registration(&registration_body(
            "registered-guest",
            "registered-guest@example.com",
            "12345678910",
            registration_code,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let guest: serde_json::Value = response.json().await.unwrap();
    assert_eq!(guest["username"], "registered-guest");
    assert_eq!(guest["email"], "registered-guest@example.com");
    assert_eq!(guest["phone_number"], "12345678910");
    assert_eq!(guest["user_type"]["name"], "guest");
    assert_eq!(
        guest["laboratory"]["laboratory_id"],
        laboratory_id.to_string()
    );
    assert_eq!(app.get_me().await.status(), StatusCode::UNAUTHORIZED);

    let password_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE username = 'registered-guest'")
            .fetch_one(&app.db_pool)
            .await
            .unwrap();
    assert_ne!(password_hash, "password");
    assert!(password_hash.starts_with("$argon2"));

    let response = app
        .post_login(&serde_json::json!({
            "username": "registered-guest",
            "password": "password",
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn replacement_expiry_and_single_use_are_enforced() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Code Lifecycle Lab").await;
    let issuer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&issuer).await;
    let first = issue_code(&app, &issuer, laboratory_id).await;
    let second = issue_code(&app, &issuer, laboratory_id).await;

    let response = app
        .post_guest_registration(&registration_body(
            "old-code-guest",
            "old-code@example.com",
            "12345678911",
            first["registration_code"].as_str().unwrap(),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let second_code = second["registration_code"].as_str().unwrap();
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "single-use-guest",
            "single-use@example.com",
            "12345678912",
            second_code,
        ))
        .await
        .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "reuse-guest",
            "reuse@example.com",
            "12345678913",
            second_code,
        ))
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let expired = issue_code(&app, &issuer, laboratory_id).await;
    let expired_id: Uuid = expired["registration_code_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query(
        r#"
        UPDATE guest_registration_codes
        SET created_at = now() - interval '20 minutes',
            expires_at = now() - interval '10 minutes'
        WHERE registration_code_id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "expired-guest",
            "expired@example.com",
            "12345678914",
            expired["registration_code"].as_str().unwrap(),
        ))
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn identity_conflict_rolls_back_code_consumption() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Conflict Lab").await;
    let issuer = TestUser::generate_with_user_type("lab_admin", Some(laboratory_id));
    app.store_user(&issuer).await;
    let existing = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&existing).await;
    let code = issue_code(&app, &issuer, laboratory_id).await;
    let registration_code = code["registration_code"].as_str().unwrap();

    assert_eq!(
        app.post_guest_registration(&serde_json::json!({
            "username": "unknown-field-guest",
            "password": "password",
            "email": "unknown-field@example.com",
            "phone_number": "12345678915",
            "registration_code": registration_code,
            "user_type": "user",
        }))
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.post_guest_registration(&registration_body(
            &existing.username,
            "conflict@example.com",
            "12345678916",
            registration_code,
        ))
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "corrected-guest",
            "corrected@example.com",
            "12345678917",
            registration_code,
        ))
        .await
        .status(),
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn code_becomes_invalid_when_issuer_loses_eligibility_or_is_deleted() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Issuer Lifecycle Lab").await;
    let issuer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&issuer).await;
    let code = issue_code(&app, &issuer, laboratory_id).await;

    sqlx::query(
        r#"
        UPDATE users
        SET user_type_id = (SELECT user_type_id FROM user_types WHERE name = 'guest')
        WHERE user_id = $1
        "#,
    )
    .bind(issuer.user_id)
    .execute(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "ineligible-issuer-guest",
            "ineligible@example.com",
            "12345678917",
            code["registration_code"].as_str().unwrap(),
        ))
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let second_issuer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&second_issuer).await;
    let second_code = issue_code(&app, &second_issuer, laboratory_id).await;
    sqlx::query("DELETE FROM users WHERE user_id = $1")
        .bind(second_issuer.user_id)
        .execute(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(
        app.post_guest_registration(&registration_body(
            "deleted-issuer-guest",
            "deleted@example.com",
            "12345678918",
            second_code["registration_code"].as_str().unwrap(),
        ))
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn concurrent_registration_consumes_a_code_only_once() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Guest Concurrent Lab").await;
    let issuer = TestUser::generate_with_user_type("user", Some(laboratory_id));
    app.store_user(&issuer).await;
    let code = issue_code(&app, &issuer, laboratory_id).await;
    let registration_code = code["registration_code"].as_str().unwrap();
    let first = registration_body(
        "concurrent-guest-one",
        "concurrent-one@example.com",
        "12345678919",
        registration_code,
    );
    let second = registration_body(
        "concurrent-guest-two",
        "concurrent-two@example.com",
        "12345678920",
        registration_code,
    );

    let (first_response, second_response) = tokio::join!(
        app.post_guest_registration(&first),
        app.post_guest_registration(&second)
    );
    let mut statuses = [first_response.status(), second_response.status()];
    statuses.sort_by_key(StatusCode::as_u16);
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::BAD_REQUEST]);

    let guest_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM users
        INNER JOIN user_types USING (user_type_id)
        WHERE laboratory_id = $1
          AND user_types.name = 'guest'
        "#,
    )
    .bind(laboratory_id)
    .fetch_one(&app.db_pool)
    .await
    .unwrap();
    assert_eq!(guest_count, 1);
}

#[tokio::test]
async fn public_registration_is_limited_to_ten_requests_per_ip() {
    let app = spawn_app().await;
    for _ in 0..10 {
        let response = app
            .post_guest_registration(&serde_json::json!({ "registration_code": "bad" }))
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let response = app
        .post_guest_registration(&serde_json::json!({ "registration_code": "bad" }))
        .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after: u64 = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=600).contains(&retry_after));
}

#[test]
fn registration_codes_are_not_plain_secrets_in_debug_output() {
    let code =
        lab_inventory::domain::GuestRegistrationCode::parse(Secret::new("123456".into())).unwrap();
    assert!(!format!("{code:?}").contains(code.as_ref().expose_secret()));
}
