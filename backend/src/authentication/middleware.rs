use crate::session_state::TypedSession;
use crate::utils::{e500, json_unauthorized};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{Payload, ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage, HttpRequest};
use std::future::{Ready, ready};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub struct UserId(Uuid);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequest for UserId {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        match req.extensions().get::<UserId>() {
            Some(user_id) => ready(Ok(*user_id)),
            None => {
                let e = anyhow::anyhow!("User id was not found in request extensions");
                ready(Err(InternalError::from_response(
                    e,
                    json_unauthorized("Authentication required"),
                )
                .into()))
            }
        }
    }
}

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
