use crate::access_control::{Actor, get_actor};
use crate::session_state::TypedSession;
use crate::utils::{e500, json_unauthorized};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage, web};
use sqlx::PgPool;

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, actix_web::Error> {
    let session = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;

    let Some(user_id) = session.get_user_id().map_err(e500)? else {
        return Ok(unauthorized_response(req));
    };
    let pool = req
        .app_data::<web::Data<PgPool>>()
        .ok_or_else(|| e500("Database pool is not configured"))?;
    let Some(actor) = get_actor(pool, user_id).await.map_err(e500)? else {
        session.log_out();
        return Ok(unauthorized_response(req));
    };

    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert::<Actor>(actor);
    next.call(req)
        .await
        .map(ServiceResponse::map_into_left_body)
}

pub async fn reject_non_laboratory_users(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, actix_web::Error> {
    let allowed = req
        .extensions()
        .get::<Actor>()
        .map(|actor| actor.laboratory_id.is_some() && !actor.is_system_admin())
        .unwrap_or(false);
    if !allowed {
        return Ok(forbidden_response(
            req,
            "A laboratory-scoped user is required",
        ));
    }
    next.call(req)
        .await
        .map(ServiceResponse::map_into_left_body)
}

pub async fn reject_non_system_admins(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, actix_web::Error> {
    let allowed = req
        .extensions()
        .get::<Actor>()
        .map(Actor::is_system_admin)
        .unwrap_or(false);
    if !allowed {
        return Ok(forbidden_response(
            req,
            "System administrator permissions are required",
        ));
    }
    next.call(req)
        .await
        .map(ServiceResponse::map_into_left_body)
}

fn unauthorized_response<B>(req: ServiceRequest) -> ServiceResponse<EitherBody<B>> {
    req.into_response(json_unauthorized("Authentication required"))
        .map_into_right_body()
}

fn forbidden_response<B>(
    req: ServiceRequest,
    message: &'static str,
) -> ServiceResponse<EitherBody<B>> {
    req.into_response(
        actix_web::HttpResponse::Forbidden().json(serde_json::json!({ "error": message })),
    )
    .map_into_right_body()
}
