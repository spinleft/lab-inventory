mod cancel;
mod create;
mod list;
mod model;
mod queries;
mod resolve;
mod service;

pub use cancel::cancel_borrow_request;
pub use create::create_borrow_request;
pub use list::{list_borrow_requests, list_my_borrow_requests};
pub use resolve::resolve_borrow_request;
pub use service::BorrowRequestError;

// Federation files and cancels borrow requests on behalf of a remote user, so it
// needs the flows themselves rather than the handlers wrapping them. `service` is
// private to this module, so these have to be re-exported to be reachable at all.
pub(crate) use model::MyBorrowRequestResponse;
pub(crate) use service::{
    cancel_borrow_request_in_transaction, create_borrow_request_in_transaction,
    list_borrow_requests_for_guest_link, validate_borrow_request_status,
};
