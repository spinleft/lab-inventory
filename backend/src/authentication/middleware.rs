use crate::domain::UserId;
use crate::session_state::TypedSession;
use crate::utils::{e500, json_unauthorized};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage};

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, actix_web::Error> {
    let session = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;

    match session.get_user_id().map_err(e500)? {
        Some(user_id) => {
            req.extensions_mut().insert(UserId(user_id));
            next.call(req)
                .await
                .map(ServiceResponse::map_into_left_body)
        }
        None => Ok(unauthorized_response(req)),
    }
}

fn unauthorized_response<B>(req: ServiceRequest) -> ServiceResponse<EitherBody<B>> {
    req.into_response(json_unauthorized("Authentication required"))
        .map_into_right_body()
}
