use crate::utils::json_unauthorized;
use actix_web::dev::Payload;
use actix_web::error::InternalError;
use actix_web::{FromRequest, HttpMessage, HttpRequest};
use std::future::{Ready, ready};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

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
