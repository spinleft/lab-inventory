use crate::helpers::spawn_app;
use lab_inventory::bootstrap::{
    apply_initial_root_password, ensure_root_password_rotated, set_password,
};
use secrecy::Secret;

#[tokio::test]
async fn a_freshly_migrated_database_fails_the_root_password_check() {
    let app = spawn_app().await;

    let outcome = ensure_root_password_rotated(&app.db_pool).await;

    assert!(outcome.is_err());
}

#[tokio::test]
async fn rotating_the_root_password_satisfies_the_check() {
    let app = spawn_app().await;

    let applied = apply_initial_root_password(
        &app.db_pool,
        Secret::new("a-password-nobody-published".to_string()),
    )
    .await
    .unwrap();

    assert!(applied);
    assert!(ensure_root_password_rotated(&app.db_pool).await.is_ok());
}

/// The initial password lives in a deployment's environment file and is read on
/// every start, so it must not overwrite a password the operator later chose.
#[tokio::test]
async fn the_initial_root_password_is_only_applied_once() {
    let app = spawn_app().await;
    apply_initial_root_password(&app.db_pool, Secret::new("the-first-password".to_string()))
        .await
        .unwrap();
    let after_first = stored_root_hash(&app).await;

    let applied = apply_initial_root_password(
        &app.db_pool,
        Secret::new("a-different-password".to_string()),
    )
    .await
    .unwrap();

    assert!(!applied);
    assert_eq!(after_first, stored_root_hash(&app).await);
}

#[tokio::test]
async fn an_initial_root_password_that_breaks_the_password_policy_is_rejected() {
    let app = spawn_app().await;

    let outcome = apply_initial_root_password(&app.db_pool, Secret::new("short".to_string())).await;

    assert!(outcome.is_err());
    assert!(ensure_root_password_rotated(&app.db_pool).await.is_err());
}

/// Resetting `root` back to the published password has to count as unrotated,
/// even though the stored hash carries a fresh salt and so differs textually
/// from the seeded one.
#[tokio::test]
async fn re_hashing_the_published_password_does_not_pass_the_check() {
    let app = spawn_app().await;

    set_password(
        &app.db_pool,
        "root",
        Secret::new("everythinghastostartsomewhere".to_string()),
    )
    .await
    .unwrap();

    assert_ne!(stored_root_hash(&app).await, SEEDED_HASH);
    assert!(ensure_root_password_rotated(&app.db_pool).await.is_err());
}

#[tokio::test]
async fn setting_the_password_of_an_unknown_user_reports_no_match() {
    let app = spawn_app().await;

    let updated = set_password(
        &app.db_pool,
        "nobody-by-that-name",
        Secret::new("a-perfectly-fine-password".to_string()),
    )
    .await
    .unwrap();

    assert!(!updated);
}

/// A fresh install must not come up carrying the names of the laboratory this
/// project was first written for.
#[tokio::test]
async fn a_fresh_database_carries_no_named_demo_laboratories() {
    let app = spawn_app().await;

    let laboratories: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT laboratory_id, name FROM laboratories")
            .fetch_all(&app.db_pool)
            .await
            .unwrap();

    // The surviving laboratory owns the seeded units, so it is renamed rather
    // than deleted.
    assert_eq!(laboratories.len(), 1);
    let (laboratory_id, name) = &laboratories[0];
    assert_eq!(
        laboratory_id.to_string(),
        "7227c5ab-78ef-43ce-87bc-5ce2337ccfe3"
    );
    assert_eq!(name, "默认实验室");

    let categories: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset_categories")
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(categories, 0);

    // Whereas the units it carries are a usable starting point, and stay.
    let units: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM units")
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(units, 5);
}

const SEEDED_HASH: &str = lab_inventory::bootstrap::SEEDED_ROOT_PASSWORD_HASH;

async fn stored_root_hash(app: &crate::helpers::TestApp) -> String {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE username = 'root'")
        .fetch_one(&app.db_pool)
        .await
        .unwrap()
}
