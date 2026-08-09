mod assign;
mod delete;
mod download;
mod get;
mod list;
mod model;
mod update;

pub use assign::{assign_asset_attachment, assign_inventory_item_attachment};
pub(crate) use assign::{AssignAttachmentError, AttachmentJsonData, assign_uploaded_attachments};
pub(crate) use model::AttachmentTarget;
pub use delete::delete_attachment;
pub use download::download_attachment;
pub use get::get_attachment;
pub use list::{
    list_asset_attachments, list_inventory_item_attachments, list_laboratory_attachments,
};
pub use update::update_attachment;
