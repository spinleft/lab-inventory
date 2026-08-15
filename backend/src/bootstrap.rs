//! First-boot concerns: applying the schema and retiring the seeded `root`
//! account.
//!
//! Both matter only to real deployments. Tests build their databases through
//! `sqlx::migrate!` directly and never log in as `root`, so everything here is
//! off unless configuration turns it on.

use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

use crate::authentication::hash_password;
use crate::domain::UserPassword;

/// The password hash the initial migration writes for `root`.
///
/// It is committed in this repository, so the password behind it is public
/// knowledge. A deployment that still carries it has no protected admin
/// account at all, which is what [`ensure_root_password_rotated`] refuses to
/// let happen.
pub const SEEDED_ROOT_PASSWORD_HASH: &str = "$argon2id$v=19$m=15000,t=2,p=1$OEx/rcq+3ts//WUDzGNl2g$Am8UFBA4w5NJEmAtquGvBmAlu92q/VQcaoL5AyJPfc8";

/// Applies every migration the database is missing.
///
/// The migrations are embedded in the binary at compile time, so a release
/// image needs neither `sqlx-cli` nor the `migrations` directory to upgrade
/// its own schema. `sqlx` takes a Postgres advisory lock for the duration,
/// which makes concurrent starts safe.
#[tracing::instrument(name = "Run pending migrations", skip(pool))]
pub async fn run_pending_migrations(pool: &PgPool) -> Result<(), anyhow::Error> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("Failed to apply database migrations.")?;
    tracing::info!("Database schema is up to date.");
    Ok(())
}

/// Replaces the seeded `root` password with `password`, but only while the
/// seeded one is still in place.
///
/// Returns whether it changed anything. Doing nothing once the password has
/// been rotated is the point: the setting that carries the initial password
/// stays in the deployment's environment file, and re-reading it on every
/// restart must not undo a password the operator has since changed.
#[tracing::instrument(name = "Apply initial root password", skip(pool, password))]
pub async fn apply_initial_root_password(
    pool: &PgPool,
    password: Secret<String>,
) -> Result<bool, anyhow::Error> {
    let password = UserPassword::parse(password).map_err(|error| {
        anyhow::anyhow!("`application.initial_root_password` is invalid: {error}")
    })?;

    if !root_password_is_seeded(pool).await? {
        tracing::info!(
            "The root password has already been changed; \
             the configured initial password was ignored."
        );
        return Ok(false);
    }
    set_password(pool, "root", password.0).await?;
    tracing::info!("The seeded root password was replaced with the configured initial password.");
    Ok(true)
}

/// Fails when `root` still carries the password seeded by the migrations.
#[tracing::instrument(name = "Check root password rotation", skip(pool))]
pub async fn ensure_root_password_rotated(pool: &PgPool) -> Result<(), anyhow::Error> {
    if !root_password_is_seeded(pool).await? {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "The `root` account still uses the password seeded by the migrations, which is \
         published in this project's source. Set a password before exposing this server:\n\
         \n  APP_APPLICATION__INITIAL_ROOT_PASSWORD=<password> (applied on the next start)\n\
         \nor run the admin CLI against this database:\n\
         \n  lab-inventory-admin set-password root\n\
         \nSet `application.require_root_password_rotation` to `false` to start anyway."
    ))
}

/// Sets `username`'s password, returning whether such a user existed.
#[tracing::instrument(name = "Set user password", skip(pool, password))]
pub async fn set_password(
    pool: &PgPool,
    username: &str,
    password: Secret<String>,
) -> Result<bool, anyhow::Error> {
    let password_hash = hash_password(password).await?;
    let result = sqlx::query!(
        r#"
        UPDATE users
        SET password_hash = $1
        WHERE username = $2
        "#,
        password_hash.expose_secret(),
        username,
    )
    .execute(pool)
    .await
    .context("Failed to store the new password.")?;
    Ok(result.rows_affected() > 0)
}

/// Whether `root` still verifies against the seeded password hash.
///
/// The stored hash is compared by verification rather than by string equality:
/// re-hashing the same password yields a different salt, so a deployment that
/// merely *reset* `root` to the published default would slip past an equality
/// check.
async fn root_password_is_seeded(pool: &PgPool) -> Result<bool, anyhow::Error> {
    let stored = sqlx::query!(
        r#"
        SELECT password_hash
        FROM users
        WHERE username = 'root'
        "#
    )
    .fetch_optional(pool)
    .await
    .context("Failed to read the root password hash.")?;

    // No `root` row means the operator removed the seeded account outright,
    // which is a stronger position than rotating its password.
    let Some(stored) = stored else {
        return Ok(false);
    };

    if stored.password_hash == SEEDED_ROOT_PASSWORD_HASH {
        return Ok(true);
    }
    Ok(verifies_against_seeded_password(&stored.password_hash))
}

/// Whether `password_hash` accepts the password behind [`SEEDED_ROOT_PASSWORD_HASH`].
fn verifies_against_seeded_password(password_hash: &str) -> bool {
    // Recovered from the seeded hash, which this repository publishes.
    const SEEDED_ROOT_PASSWORD: &str = "everythinghastostartsomewhere";

    let Ok(parsed) = PasswordHash::new(password_hash) else {
        // An unparseable hash cannot be the seeded one, and rejecting it here
        // would block startup over something this check has no business
        // judging.
        return false;
    };
    Argon2::default()
        .verify_password(SEEDED_ROOT_PASSWORD.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seeded_hash_accepts_the_published_password() {
        assert!(verifies_against_seeded_password(SEEDED_ROOT_PASSWORD_HASH));
    }

    #[test]
    fn a_rotated_hash_is_not_mistaken_for_the_seeded_one() {
        let rotated = crate::authentication::compute_password_hash(Secret::new(
            "a-freshly-chosen-password".to_string(),
        ))
        .unwrap();
        assert!(!verifies_against_seeded_password(rotated.expose_secret()));
    }

    #[test]
    fn an_unparseable_hash_is_not_treated_as_seeded() {
        assert!(!verifies_against_seeded_password("not-a-phc-string"));
    }
}
