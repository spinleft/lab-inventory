//! The read-only view of a laboratory that federated partners are served.
//!
//! It is a separate API from the local routes rather than a filter over them:
//! everything here is desensitized by construction, so a partner can never be
//! handed internal notes or a non-public attachment.
mod model;
mod queries;
mod respond;
mod service;

pub(super) use model::PublicDataError;
pub(super) use respond::{parse_read_target, respond_public_data};
