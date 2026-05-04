use crate::telemetry::spawn_blocking_with_tracing;
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Authentication failed")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[derive(sqlx::FromRow)]
struct StoredCredentials {
    user_id: uuid::Uuid,
    password_hash: String,
}

#[derive(sqlx::FromRow)]
struct StoredPassword {
    password_hash: String,
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<StoredCredentials>, anyhow::Error> {
    let row = sqlx::query_as::<_, StoredCredentials>(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("Failed to retrieve stored credentials.")?;
    Ok(row)
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, AuthError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    if let Some(stored_credentials) = get_stored_credentials(&credentials.username, pool).await? {
        user_id = Some(stored_credentials.user_id);
        expected_password_hash = Secret::new(stored_credentials.password_hash);
    }

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")??;

    user_id
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Validate password for user", skip(password, pool), fields(user_id=%user_id))]
pub async fn validate_password_for_user(
    user_id: uuid::Uuid,
    password: Secret<String>,
    pool: &PgPool,
) -> Result<(), AuthError> {
    let stored_password = sqlx::query_as::<_, StoredPassword>(
        r#"
        SELECT password_hash
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to retrieve stored password.")?
    .ok_or_else(|| anyhow::anyhow!("Unknown user id."))
    .map_err(AuthError::InvalidCredentials)?;

    spawn_blocking_with_tracing(move || {
        verify_password_hash(Secret::new(stored_password.password_hash), password)
    })
    .await
    .context("Failed to spawn blocking task.")??;

    Ok(())
}

pub async fn hash_password(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    spawn_blocking_with_tracing(move || compute_password_hash(password))
        .await?
        .context("Failed to hash password")
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse password hash in PHC string format.")?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    use argon2::password_hash::SaltString;
    use argon2::{Algorithm, PasswordHasher, Version};

    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        argon2::Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)?
    .to_string();
    Ok(Secret::new(password_hash))
}
