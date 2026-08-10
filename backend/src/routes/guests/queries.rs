//! Every SQL statement used by guest registration lives here.
//!
//! Each function issues at most one statement and leaves orchestration to the
//! handlers. Registration code consumption and user creation share a caller-
//! owned transaction so a failed user insert cannot burn a valid code.
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct RegistrationCodeRow {
    pub(super) registration_code_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) created_by_user_id: Uuid,
    pub(super) expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct RevokedRegistrationCodeRow {
    pub(super) registration_code_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) expires_at: DateTime<Utc>,
}

pub(super) async fn lock_laboratory(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
) -> Result<bool, anyhow::Error> {
    let laboratory_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT laboratory_id
        FROM laboratories
        WHERE laboratory_id = $1
        FOR UPDATE
        "#,
    )
    .bind(laboratory_id)
    .fetch_optional(transaction.as_mut())
    .await?;

    Ok(laboratory_id.is_some())
}

pub(super) async fn revoke_expired_registration_code(
    transaction: &mut Transaction<'_, Postgres>,
    code_hmac: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        r#"
        UPDATE guest_registration_codes
        SET revoked_at = now()
        WHERE code_hmac = $1
          AND consumed_at IS NULL
          AND revoked_at IS NULL
          AND expires_at <= now()
        "#,
    )
    .bind(code_hmac)
    .execute(transaction.as_mut())
    .await?;
    Ok(())
}

pub(super) async fn revoke_laboratory_registration_code(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
) -> Result<Option<RevokedRegistrationCodeRow>, anyhow::Error> {
    sqlx::query_as::<_, RevokedRegistrationCodeRow>(
        r#"
        UPDATE guest_registration_codes
        SET revoked_at = now()
        WHERE laboratory_id = $1
          AND consumed_at IS NULL
          AND revoked_at IS NULL
        RETURNING registration_code_id, laboratory_id, expires_at
        "#,
    )
    .bind(laboratory_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(Into::into)
}

pub(super) async fn insert_registration_code(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    code_hmac: &str,
    created_by_user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<Option<RegistrationCodeRow>, anyhow::Error> {
    sqlx::query_as::<_, RegistrationCodeRow>(
        r#"
        INSERT INTO guest_registration_codes (
            registration_code_id,
            laboratory_id,
            code_hmac,
            created_by_user_id,
            expires_at
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT DO NOTHING
        RETURNING registration_code_id, laboratory_id, created_by_user_id, expires_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(code_hmac)
    .bind(created_by_user_id)
    .bind(expires_at)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(Into::into)
}

/// Claims a code while locking both it and its issuer. Rechecking the issuer's
/// current role and laboratory prevents a stale invitation from surviving a
/// role or laboratory change.
pub(super) async fn consume_registration_code(
    transaction: &mut Transaction<'_, Postgres>,
    code_hmac: &str,
) -> Result<Option<RegistrationCodeRow>, anyhow::Error> {
    sqlx::query_as::<_, RegistrationCodeRow>(
        r#"
        WITH candidate AS (
            SELECT
                codes.registration_code_id,
                codes.laboratory_id,
                codes.created_by_user_id,
                codes.expires_at
            FROM guest_registration_codes AS codes
            INNER JOIN users AS issuer
                ON issuer.user_id = codes.created_by_user_id
               AND issuer.laboratory_id = codes.laboratory_id
            INNER JOIN user_types AS issuer_type
                ON issuer_type.user_type_id = issuer.user_type_id
               AND issuer_type.name IN ('lab_admin', 'user')
            WHERE codes.code_hmac = $1
              AND codes.consumed_at IS NULL
              AND codes.revoked_at IS NULL
              AND codes.expires_at > now()
            FOR UPDATE OF codes, issuer
        ),
        consumed AS (
            UPDATE guest_registration_codes AS codes
            SET consumed_at = now()
            FROM candidate
            WHERE codes.registration_code_id = candidate.registration_code_id
            RETURNING
                codes.registration_code_id,
                codes.laboratory_id,
                codes.created_by_user_id,
                codes.expires_at
        )
        SELECT registration_code_id, laboratory_id, created_by_user_id, expires_at
        FROM consumed
        "#,
    )
    .bind(code_hmac)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(Into::into)
}
