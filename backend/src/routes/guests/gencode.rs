use super::queries::{
    RegistrationCodeRow, RevokedRegistrationCodeRow, insert_registration_code, lock_laboratory,
    revoke_expired_registration_code, revoke_laboratory_registration_code,
};
use super::register::GuestRegistrationError;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::GuestRegistrationHasher;
use crate::domain::GuestRegistrationCode;
use actix_web::{HttpResponse, web};
use anyhow::{Context, anyhow};
use chrono::{Duration, Utc};
use secrecy::ExposeSecret;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const REGISTRATION_CODE_TTL_MINUTES: i64 = 10;
const REGISTRATION_CODE_GENERATION_ATTEMPTS: usize = 32;

#[derive(Serialize)]
struct RegistrationCodeResponse {
    registration_code_id: Uuid,
    registration_code: String,
    laboratory_id: Uuid,
    expires_at: chrono::DateTime<Utc>,
}

#[tracing::instrument(
    name = "Creating a guest registration code",
    skip(pool, hasher),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_guest_registration_code(
    pool: web::Data<PgPool>,
    hasher: web::Data<GuestRegistrationHasher>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, GuestRegistrationError> {
    let actor = laboratory_context.actor();
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    if !validate_permission(
        &pool,
        actor,
        ResourceType::GuestRegistrationCode,
        Action::Create(laboratory_id),
    )
    .await?
    {
        return Err(GuestRegistrationError::Forbidden(
            "You don't have permission to create a guest registration code.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to begin guest registration code transaction")?;
    if !lock_laboratory(&mut transaction, laboratory_id).await? {
        return Err(GuestRegistrationError::ValidationError(
            "Invalid laboratory".into(),
        ));
    }
    let replaced = revoke_laboratory_registration_code(&mut transaction, laboratory_id).await?;
    let expires_at = Utc::now() + Duration::minutes(REGISTRATION_CODE_TTL_MINUTES);

    let mut generated = None;
    for _ in 0..REGISTRATION_CODE_GENERATION_ATTEMPTS {
        let code = GuestRegistrationCode::generate();
        let code_hmac = hasher.hash_code(&code);
        revoke_expired_registration_code(&mut transaction, &code_hmac).await?;
        if let Some(row) = insert_registration_code(
            &mut transaction,
            laboratory_id,
            &code_hmac,
            *actor.user_id,
            expires_at,
        )
        .await?
        {
            generated = Some((code, row));
            break;
        }
    }
    let (registration_code, row) = generated.ok_or_else(|| {
        GuestRegistrationError::UnexpectedError(anyhow!(
            "Failed to allocate a unique guest registration code"
        ))
    })?;

    if let Some(replaced) = replaced {
        record_audit(
            &mut transaction,
            actor,
            AuditAction::Update,
            AuditResource::GuestRegistrationCode,
            Some(replaced.registration_code_id),
            replaced_code_audit_details(&replaced),
        )
        .await?;
    }
    record_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        AuditResource::GuestRegistrationCode,
        Some(row.registration_code_id),
        created_code_audit_details(&row),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit guest registration code transaction")?;

    Ok(HttpResponse::Created().json(RegistrationCodeResponse {
        registration_code_id: row.registration_code_id,
        registration_code: registration_code.as_ref().expose_secret().clone(),
        laboratory_id: row.laboratory_id,
        expires_at: row.expires_at,
    }))
}

fn created_code_audit_details(code: &RegistrationCodeRow) -> Value {
    json!({
        "laboratory_id": code.laboratory_id,
        "expires_at": code.expires_at,
        "rollback": {
            "operation": "delete",
            "resource_type": "guest_registration_code",
            "where": { "registration_code_id": code.registration_code_id },
        },
    })
}

fn replaced_code_audit_details(code: &RevokedRegistrationCodeRow) -> Value {
    json!({
        "laboratory_id": code.laboratory_id,
        "expires_at": code.expires_at,
        "reason": "replaced",
    })
}
