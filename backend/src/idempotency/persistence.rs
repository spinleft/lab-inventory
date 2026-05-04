use super::IdempotencyKey;
use crate::utils::ApiError;
use actix_web::HttpResponse;
use actix_web::body::to_bytes;
use actix_web::http::StatusCode;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

#[derive(sqlx::FromRow)]
struct SavedResponseRecord {
    response_status_code: Option<i16>,
    response_headers: Option<Vec<HeaderPairRecord>>,
    response_body: Option<Vec<u8>>,
}

pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<HttpResponse>, ApiError> {
    let saved_response = sqlx::query_as::<_, SavedResponseRecord>(
        r#"
        SELECT response_status_code, response_headers, response_body
        FROM idempotency
        WHERE user_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(user_id)
    .bind(idempotency_key.as_ref())
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    match saved_response {
        Some(response) => {
            let status_code = response
                .response_status_code
                .ok_or_else(|| ApiError::Conflict("Request is still being processed".into()))?;
            let response_headers = response
                .response_headers
                .ok_or_else(|| ApiError::Conflict("Request is still being processed".into()))?;
            let response_body = response
                .response_body
                .ok_or_else(|| ApiError::Conflict("Request is still being processed".into()))?;
            let status_code = StatusCode::from_u16(status_code as u16)
                .map_err(|e| ApiError::UnexpectedError(e.into()))?;

            let mut response = HttpResponse::build(status_code);
            for HeaderPairRecord { name, value } in response_headers {
                response.append_header((name, value));
            }
            Ok(Some(response.body(response_body)))
        }
        None => Ok(None),
    }
}

pub async fn save_response(
    mut transaction: Transaction<'_, Postgres>,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_response: HttpResponse,
) -> Result<HttpResponse, ApiError> {
    let (response_head, body) = http_response.into_parts();
    let body = to_bytes(body)
        .await
        .map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!("{e}")))?;
    let status_code = response_head.status().as_u16() as i16;
    let headers = {
        let mut h = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    transaction
        .execute(
            sqlx::query(
                r#"
                UPDATE idempotency
                SET
                    response_status_code = $3,
                    response_headers = $4,
                    response_body = $5
                WHERE user_id = $1 AND idempotency_key = $2
                "#,
            )
            .bind(user_id)
            .bind(idempotency_key.as_ref())
            .bind(status_code)
            .bind(headers)
            .bind(body.as_ref()),
        )
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}

#[allow(clippy::large_enum_variant)]
pub enum NextAction<'a> {
    StartProcessing(Transaction<'a, Postgres>),
    ReturnSavedResponse(HttpResponse),
}

pub async fn try_processing<'a>(
    pool: &'a PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction<'a>, ApiError> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let inserted = transaction
        .execute(
            sqlx::query(
                r#"
                INSERT INTO idempotency (user_id, idempotency_key, created_at)
                VALUES ($1, $2, now())
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(user_id)
            .bind(idempotency_key.as_ref()),
        )
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .rows_affected();

    if inserted > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| {
                ApiError::UnexpectedError(anyhow::anyhow!(
                    "We expected a saved response, we didn't find it"
                ))
            })?;
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}
