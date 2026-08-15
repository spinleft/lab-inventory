use serde::Deserialize;
use uuid::Uuid;

/// What a federated caller is asking to do. Only these three operations are
/// reachable from outside; anything else the borrow module offers — approving,
/// rejecting, reading the laboratory's whole queue — stays with the laboratory
/// that owns the item.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FederationBorrowTarget {
    /// `GET borrow-requests`
    ListMine,
    /// `POST inventory-items/{inventory_item_id}/borrow-requests`
    Create(Uuid),
    /// `POST borrow-requests/{borrow_request_id}/cancel`
    Cancel(Uuid),
}

/// The wire format of an inbound borrow request.
///
/// Deliberately its own type rather than the one the session route deserializes:
/// this one is a contract with other servers, and it should not shift because a
/// local payload grew a field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FederationCreateBorrowRequestBody {
    pub(super) request_note: Option<String>,
}
