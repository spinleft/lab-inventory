#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AttachmentFileSize(i64);

impl AttachmentFileSize {
    pub fn parse(size: i64) -> Result<Self, String> {
        if size > 0 {
            Ok(Self(size))
        } else {
            Err("attachment file size must be positive".into())
        }
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl From<AttachmentFileSize> for i64 {
    fn from(size: AttachmentFileSize) -> Self {
        size.0
    }
}
