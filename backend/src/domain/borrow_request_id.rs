use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BorrowRequestId(Uuid);

impl BorrowRequestId {
    pub fn parse(id: Uuid) -> Result<Self, String> {
        Ok(Self(id))
    }
}

impl std::fmt::Display for BorrowRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Uuid> for BorrowRequestId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for BorrowRequestId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<BorrowRequestId> for Uuid {
    fn from(borrow_request_id: BorrowRequestId) -> Self {
        borrow_request_id.0
    }
}

impl From<Uuid> for BorrowRequestId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}
