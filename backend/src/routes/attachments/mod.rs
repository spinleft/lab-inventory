mod assign;
mod delete;
mod download;
mod get;
mod list;
mod model;
mod queries;
mod service;
mod update;

pub(crate) use assign::AttachmentJsonData;
pub use assign::{assign_asset_attachment, assign_inventory_item_attachment};
pub use delete::delete_attachment;
pub use download::download_attachment;
pub use get::get_attachment;
pub use list::{
    list_asset_attachments, list_inventory_item_attachments, list_laboratory_attachments,
};
pub(crate) use model::AttachmentTarget;
pub(crate) use service::{AssignAttachmentError, assign_uploaded_attachments};
pub use update::update_attachment;
