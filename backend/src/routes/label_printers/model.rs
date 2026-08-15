use crate::domain::LabelPrinterMedia;
use crate::label_printing::{MAX_RASTER_LINES, MIN_RASTER_LINES};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// Dots per inch every QL-800 family printer rasterises at.
const PRINTER_DPI: u16 = 300;

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct LabelPrinterRow {
    pub(super) printer_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) name: String,
    pub(super) host: String,
    pub(super) port: i32,
    pub(super) model: String,
    pub(super) media_kind: String,
    pub(super) media_width_mm: i32,
    pub(super) media_length_mm: Option<i32>,
    pub(super) auto_cut: bool,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

impl LabelPrinterRow {
    /// Resolves the configured stock to a layout the raster layer can use.
    ///
    /// Returns `None` when the row names a size this build does not know how to
    /// place, which the create and update paths make impossible but a
    /// hand-edited row could still produce.
    pub(super) fn media(&self) -> Option<LabelPrinterMedia> {
        LabelPrinterMedia::parse(&self.media_kind, self.media_width_mm, self.media_length_mm).ok()
    }
}

/// Everything a client needs to lay a label out at the right size.
///
/// The client renders the bitmap, so it has to be told the exact dot dimensions
/// the printer will accept — otherwise every print would be a guess that the
/// server rejects.
#[derive(Serialize)]
pub(super) struct LabelLayout {
    dpi: u16,
    printable_width_dots: u16,
    /// Zero for continuous stock, where the client chooses the length.
    printable_length_dots: u16,
    min_length_dots: u32,
    max_length_dots: u32,
}

impl From<LabelPrinterMedia> for LabelLayout {
    fn from(media: LabelPrinterMedia) -> Self {
        let spec = media.spec();
        Self {
            dpi: PRINTER_DPI,
            printable_width_dots: spec.printable_width_dots,
            printable_length_dots: spec.printable_length_dots,
            min_length_dots: MIN_RASTER_LINES,
            max_length_dots: MAX_RASTER_LINES,
        }
    }
}

#[derive(Serialize)]
pub(super) struct LabelPrinterResponse {
    printer_id: Uuid,
    laboratory_id: Uuid,
    name: String,
    host: String,
    port: i32,
    model: String,
    media_kind: String,
    media_width_mm: i32,
    media_length_mm: Option<i32>,
    auto_cut: bool,
    /// `None` when the configured stock is not a size this build supports.
    layout: Option<LabelLayout>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<LabelPrinterRow> for LabelPrinterResponse {
    fn from(row: LabelPrinterRow) -> Self {
        let layout = row.media().map(LabelLayout::from);
        Self {
            printer_id: row.printer_id,
            laboratory_id: row.laboratory_id,
            name: row.name,
            host: row.host,
            port: row.port,
            model: row.model,
            media_kind: row.media_kind,
            media_width_mm: row.media_width_mm,
            media_length_mm: row.media_length_mm,
            auto_cut: row.auto_cut,
            layout,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_label_printer_rollback_details(printer: &LabelPrinterRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "label_printer",
            "where": {
                "printer_id": printer.printer_id,
            },
        },
    })
}

pub(super) fn update_label_printer_rollback_details(printer: &LabelPrinterRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "label_printer",
            "where": {
                "printer_id": printer.printer_id,
            },
            "values": {
                "name": &printer.name,
                "host": &printer.host,
                "port": printer.port,
                "model": &printer.model,
                "media_kind": &printer.media_kind,
                "media_width_mm": printer.media_width_mm,
                "media_length_mm": printer.media_length_mm,
                "auto_cut": printer.auto_cut,
            },
        },
    })
}

pub(super) fn delete_label_printer_rollback_details(printer: &LabelPrinterRow) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "label_printer",
            "values": {
                "printer_id": printer.printer_id,
                "laboratory_id": printer.laboratory_id,
                "name": &printer.name,
                "host": &printer.host,
                "port": printer.port,
                "model": &printer.model,
                "media_kind": &printer.media_kind,
                "media_width_mm": printer.media_width_mm,
                "media_length_mm": printer.media_length_mm,
                "auto_cut": printer.auto_cut,
                "created_at": printer.created_at,
            },
        },
    })
}

/// Audit trail for a print run. Labels are physical output, so what was printed
/// and how many of each is worth keeping even though nothing in the database
/// changed.
pub(super) fn print_labels_details(printer: &LabelPrinterRow, pages: usize, copies: u32) -> Value {
    json!({
        "printer_id": printer.printer_id,
        "printer_name": &printer.name,
        "media_kind": &printer.media_kind,
        "media_width_mm": printer.media_width_mm,
        "media_length_mm": printer.media_length_mm,
        "pages": pages,
        "copies": copies,
        "labels_printed": pages * copies as usize,
    })
}
