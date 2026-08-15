use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LabelPrinterId(pub Uuid);

impl AsRef<Uuid> for LabelPrinterId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl Deref for LabelPrinterId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LabelPrinterId> for Uuid {
    fn from(printer_id: LabelPrinterId) -> Self {
        printer_id.0
    }
}

impl From<Uuid> for LabelPrinterId {
    fn from(value: Uuid) -> Self {
        LabelPrinterId(value)
    }
}

impl std::fmt::Display for LabelPrinterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
