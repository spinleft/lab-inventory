//! What a federated partner's users may do here beyond reading: ask to borrow an
//! item, read back what they have asked for, and retract a request.
//!
//! This is a sibling of `public_data`, not part of it, and the split is the
//! point. Everything under `public_data` is answered without knowing who is
//! asking, which is what makes it safe to serve to any trusted partner.
//! Borrowing is the opposite: every operation here is scoped to one caller's
//! guest link, so folding it into `public_data` would quietly retire that
//! module's guarantee.
mod model;
mod respond;

pub(super) use respond::{parse_borrow_target, respond_federation_borrow};
