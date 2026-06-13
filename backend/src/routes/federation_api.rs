use crate::authentication::{UserId, get_actor};
use crate::federation::{signed_headers, verify_federation_request};
use crate::routes::fetch_remote_laboratory_secret;
use crate::startup::ApplicationLocalLaboratoryId;
use crate::utils::ApiError;
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
struct PublicInventoryItem {
    inventory_item_id: Uuid,
    asset_id: Uuid,
    asset_name: String,
    asset_model: Option<String>,
    laboratory_id: Uuid,
    laboratory_name: String,
    status: String,
    is_cross_lab_borrowable: bool,
    quantity_available: f64,
    unit_id: Uuid,
    unit_code: String,
    public_notes: Option<String>,
}

#[derive(Deserialize)]
pub struct RemoteBorrowPayload {
    inventory_item_id: Uuid,
    requested_quantity: f64,
    expected_borrowed_at: Option<DateTime<Utc>>,
    expected_returned_at: Option<DateTime<Utc>>,
    purpose: String,
}

#[derive(Deserialize)]
struct FederationBorrowPayload {
    correlation_id: Uuid,
    inventory_item_id: Uuid,
    requester_user_id: Uuid,
    requester_username: String,
    requester_laboratory_id: Uuid,
    requester_laboratory_name: String,
    requested_quantity: f64,
    expected_borrowed_at: Option<DateTime<Utc>>,
    expected_returned_at: Option<DateTime<Utc>>,
    purpose: String,
}

#[derive(sqlx::FromRow)]
struct BorrowableInventoryItem {
    inventory_item_id: Uuid,
    asset_name: String,
    asset_model: Option<String>,
    laboratory_id: Uuid,
    tracking_mode: String,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    unit_id: Uuid,
    unit_code: String,
    unit_allow_decimal: bool,
    is_cross_lab_borrowable: bool,
    status: String,
}

pub async fn list_federation_inventory_items(
    request: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    verify_federation_request(pool.get_ref(), &request, b"").await?;
    let items = fetch_public_inventory_items(pool.get_ref(), None).await?;
    Ok(HttpResponse::Ok().json(json!({ "items": items })))
}

pub async fn get_federation_inventory_item(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    verify_federation_request(pool.get_ref(), &request, b"").await?;
    let mut items =
        fetch_public_inventory_items(pool.get_ref(), Some(inventory_item_id.into_inner())).await?;
    let item = items.pop().ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn create_federation_borrow_request(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let federated_actor = verify_federation_request(pool.get_ref(), &request, &body).await?;
    let payload: FederationBorrowPayload =
        serde_json::from_slice(&body).map_err(|_| ApiError::BadRequest("Invalid JSON".into()))?;
    validate_borrow_payload(payload.requested_quantity, &payload.purpose)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let item = fetch_borrowable_item(&mut transaction, payload.inventory_item_id).await?;
    validate_borrowable(&item, payload.requested_quantity)?;

    let borrow_request_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO borrow_requests (
            borrow_request_id,
            correlation_id,
            direction,
            inventory_item_id,
            requester_user_id,
            requester_laboratory_id,
            owner_laboratory_id,
            requested_quantity,
            unit_id,
            expected_borrowed_at,
            expected_returned_at,
            purpose,
            remote_laboratory_id,
            remote_inventory_item_id,
            remote_requester_user_id,
            remote_requester_username,
            remote_requester_laboratory_id,
            remote_requester_laboratory_name,
            remote_asset_name,
            remote_asset_model,
            remote_unit_code
        )
        VALUES ($1, $2, 'owner_authority', $3, NULL, NULL, $4, $5, $6, $7, $8, $9, $10, $3, $11, $12, $13, $14, $15, $16, $17)
        "#,
    )
    .bind(borrow_request_id)
    .bind(payload.correlation_id)
    .bind(item.inventory_item_id)
    .bind(item.laboratory_id)
    .bind(payload.requested_quantity)
    .bind(item.unit_id)
    .bind(payload.expected_borrowed_at)
    .bind(payload.expected_returned_at)
    .bind(payload.purpose.trim())
    .bind(federated_actor.remote_laboratory_id)
    .bind(payload.requester_user_id)
    .bind(payload.requester_username.trim())
    .bind(payload.requester_laboratory_id)
    .bind(payload.requester_laboratory_name.trim())
    .bind(&item.asset_name)
    .bind(item.asset_model.as_deref())
    .bind(&item.unit_code)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(json!({
        "borrow_request_id": borrow_request_id,
        "correlation_id": payload.correlation_id,
        "status": "pending",
        "unit_id": item.unit_id,
        "unit_code": item.unit_code,
        "asset_name": item.asset_name,
        "asset_model": item.asset_model
    })))
}

pub async fn list_remote_inventory_items(
    user_id: UserId,
    pool: web::Data<PgPool>,
    local_laboratory_id: web::Data<ApplicationLocalLaboratoryId>,
    remote_laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    get_actor(pool.get_ref(), user_id).await?;
    let remote =
        fetch_remote_laboratory_secret(pool.get_ref(), remote_laboratory_id.into_inner()).await?;
    if !remote.is_enabled {
        return Err(ApiError::BadRequest("Remote laboratory is disabled".into()));
    }
    let path = "/api/v1/federation/inventory-items";
    let response = reqwest::Client::new()
        .get(format!(
            "{}{}",
            remote.api_base_url.trim_end_matches("/api/v1"),
            path
        ))
        .headers(signed_headers(
            "GET",
            path,
            b"",
            local_laboratory_id.0,
            &remote.key_id,
            &remote.shared_secret,
        )?)
        .send()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    proxy_json_response(response).await
}

pub async fn create_remote_borrow_request(
    user_id: UserId,
    pool: web::Data<PgPool>,
    local_laboratory_id: web::Data<ApplicationLocalLaboratoryId>,
    remote_laboratory_id: web::Path<Uuid>,
    payload: web::Json<RemoteBorrowPayload>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    validate_borrow_payload(payload.requested_quantity, &payload.purpose)?;
    let requester_laboratory_id = actor.laboratory_id.unwrap_or(local_laboratory_id.0);
    let requester_laboratory_name =
        laboratory_name(pool.get_ref(), requester_laboratory_id).await?;
    let remote_laboratory_id = remote_laboratory_id.into_inner();
    let remote = fetch_remote_laboratory_secret(pool.get_ref(), remote_laboratory_id).await?;
    if !remote.is_enabled {
        return Err(ApiError::BadRequest("Remote laboratory is disabled".into()));
    }

    let correlation_id = Uuid::new_v4();
    let body = serde_json::to_vec(&json!({
        "correlation_id": correlation_id,
        "inventory_item_id": payload.inventory_item_id,
        "requester_user_id": actor.user_id,
        "requester_username": "local-user",
        "requester_laboratory_id": requester_laboratory_id,
        "requester_laboratory_name": requester_laboratory_name,
        "requested_quantity": payload.requested_quantity,
        "expected_borrowed_at": payload.expected_borrowed_at,
        "expected_returned_at": payload.expected_returned_at,
        "purpose": payload.purpose.trim()
    }))
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let path = "/api/v1/federation/borrow-requests";
    let response = reqwest::Client::new()
        .post(format!(
            "{}{}",
            remote.api_base_url.trim_end_matches("/api/v1"),
            path
        ))
        .headers(signed_headers(
            "POST",
            path,
            &body,
            local_laboratory_id.0,
            &remote.key_id,
            &remote.shared_secret,
        )?)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let status = response.status();
    let remote_body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    if !status.is_success() {
        return Err(ApiError::BadRequest(
            remote_body
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("Remote request failed")
                .to_string(),
        ));
    }

    let borrow_request_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO borrow_requests (
            borrow_request_id,
            correlation_id,
            direction,
            inventory_item_id,
            requester_user_id,
            requester_laboratory_id,
            owner_laboratory_id,
            requested_quantity,
            unit_id,
            expected_borrowed_at,
            expected_returned_at,
            purpose,
            remote_laboratory_id,
            remote_inventory_item_id,
            remote_asset_name,
            remote_asset_model,
            remote_unit_code
        )
        VALUES ($1, $2, 'requester_mirror', NULL, $3, $4, NULL, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(borrow_request_id)
    .bind(correlation_id)
    .bind(actor.user_id)
    .bind(requester_laboratory_id)
    .bind(payload.requested_quantity)
    .bind(json_uuid(&remote_body, "unit_id")?)
    .bind(payload.expected_borrowed_at)
    .bind(payload.expected_returned_at)
    .bind(payload.purpose.trim())
    .bind(remote_laboratory_id)
    .bind(payload.inventory_item_id)
    .bind(remote_body.get("asset_name").and_then(|value| value.as_str()))
    .bind(remote_body.get("asset_model").and_then(|value| value.as_str()))
    .bind(remote_body.get("unit_code").and_then(|value| value.as_str()))
    .execute(pool.get_ref())
    .await
    .map_err(map_database_error)?;

    Ok(HttpResponse::Created().json(json!({
        "borrow_request_id": borrow_request_id,
        "correlation_id": correlation_id,
        "status": "pending"
    })))
}

async fn fetch_public_inventory_items(
    pool: &PgPool,
    inventory_item_id: Option<Uuid>,
) -> Result<Vec<PublicInventoryItem>, ApiError> {
    let mut query = String::from(
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.asset_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            asset_inventory_items.laboratory_id,
            laboratories.name AS laboratory_name,
            asset_inventory_items.status,
            asset_inventory_items.is_cross_lab_borrowable,
            asset_inventory_items.quantity_on_hand - asset_inventory_items.quantity_allocated AS quantity_available,
            asset_inventory_items.unit_id,
            units.code AS unit_code,
            asset_inventory_items.public_notes
        FROM asset_inventory_items
        INNER JOIN assets USING (asset_id)
        INNER JOIN laboratories ON laboratories.laboratory_id = asset_inventory_items.laboratory_id
        INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
        WHERE asset_inventory_items.is_cross_lab_borrowable = true
          AND asset_inventory_items.quantity_on_hand > asset_inventory_items.quantity_allocated
        "#,
    );
    if inventory_item_id.is_some() {
        query.push_str(" AND asset_inventory_items.inventory_item_id = $1");
    }
    query.push_str(" ORDER BY assets.name, asset_inventory_items.created_at");

    let mut sql = sqlx::query_as::<_, PublicInventoryItem>(&query);
    if let Some(inventory_item_id) = inventory_item_id {
        sql = sql.bind(inventory_item_id);
    }
    sql.fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

async fn fetch_borrowable_item(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<BorrowableInventoryItem, ApiError> {
    sqlx::query_as::<_, BorrowableInventoryItem>(
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.quantity_on_hand,
            asset_inventory_items.quantity_allocated,
            asset_inventory_items.unit_id,
            units.code AS unit_code,
            units.allow_decimal AS unit_allow_decimal,
            asset_inventory_items.is_cross_lab_borrowable,
            asset_inventory_items.status
        FROM asset_inventory_items
        INNER JOIN assets USING (asset_id)
        INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
        WHERE asset_inventory_items.inventory_item_id = $1
        FOR UPDATE
        "#,
    )
    .bind(inventory_item_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

fn validate_borrowable(
    item: &BorrowableInventoryItem,
    requested_quantity: f64,
) -> Result<(), ApiError> {
    if !item.is_cross_lab_borrowable {
        return Err(ApiError::BadRequest(
            "inventory item is not borrowable".into(),
        ));
    }
    if item.tracking_mode == "serialized" && item.status != "available" {
        return Err(ApiError::BadRequest(
            "inventory item is not available".into(),
        ));
    }
    if !item.unit_allow_decimal && requested_quantity.fract().abs() > f64::EPSILON {
        return Err(ApiError::BadRequest(
            "requested_quantity must be an integer".into(),
        ));
    }
    if requested_quantity > item.quantity_on_hand - item.quantity_allocated {
        return Err(ApiError::BadRequest(
            "requested_quantity exceeds available quantity".into(),
        ));
    }
    Ok(())
}

fn validate_borrow_payload(quantity: f64, purpose: &str) -> Result<(), ApiError> {
    if !quantity.is_finite() || quantity <= 0.0 {
        return Err(ApiError::BadRequest(
            "requested_quantity must be positive".into(),
        ));
    }
    if purpose.trim().is_empty() {
        return Err(ApiError::BadRequest("purpose is required".into()));
    }
    Ok(())
}

async fn laboratory_name(pool: &PgPool, laboratory_id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT name FROM laboratories WHERE laboratory_id = $1")
        .bind(laboratory_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::BadRequest("Unknown local laboratory".into()))
}

async fn proxy_json_response(response: reqwest::Response) -> Result<HttpResponse, ApiError> {
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    if !status.is_success() {
        return Err(ApiError::BadRequest(
            body.get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("Remote request failed")
                .to_string(),
        ));
    }
    Ok(HttpResponse::Ok().json(body))
}

fn json_uuid(body: &serde_json::Value, field: &str) -> Result<Uuid, ApiError> {
    body.get(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| ApiError::BadRequest(format!("Remote response missing {field}")))?
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("Remote response has invalid {field}")))
}

fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && let Some("23505") = database_error.code().as_deref()
    {
        return ApiError::Conflict("Borrow request already exists".into());
    }
    ApiError::UnexpectedError(error.into())
}
