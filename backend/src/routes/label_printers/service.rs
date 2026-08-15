//! Business flows that turn a stored printer row into an actual print.
use super::model::LabelPrinterRow;
use crate::configuration::LabelPrintingSettings;
use crate::domain::LabelPrinterMedia;
use crate::label_printing::raster::{self, Page, RasterError};
use crate::label_printing::transport::{AddressPolicy, PrinterEndpoint, TransportError};
use crate::label_printing::{MAX_RASTER_LINES, status::PrinterStatus};

/// Most a single request may print, across pages and copies together.
///
/// A print request is cheap to send and expensive to undo — nobody can un-eat a
/// roll of labels — so an accidental loop is capped here rather than at the
/// printer.
pub(super) const MAX_LABELS_PER_REQUEST: usize = 500;
pub(super) const MAX_COPIES: u32 = 20;

#[derive(Debug, thiserror::Error)]
pub(super) enum PrintError {
    #[error("{0}")]
    Validation(String),
    #[error("The printer is loaded with {loaded}, but this label is laid out for {expected}")]
    MediaMismatch { loaded: String, expected: String },
    #[error("The printer is not ready: {0}")]
    NotReady(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl From<RasterError> for PrintError {
    fn from(error: RasterError) -> Self {
        Self::Validation(error.to_string())
    }
}

impl LabelPrinterRow {
    pub(super) fn endpoint(&self) -> PrinterEndpoint {
        PrinterEndpoint {
            host: self.host.clone(),
            // The column is constrained to the u16 range, so this cannot wrap.
            port: self.port as u16,
        }
    }

    /// The stock this printer is configured for, or a validation error naming
    /// what is wrong with the row.
    pub(super) fn require_media(&self) -> Result<LabelPrinterMedia, PrintError> {
        self.media().ok_or_else(|| {
            PrintError::Validation(format!(
                "This printer is configured for {} {}mm label stock, which this server cannot lay out.",
                self.media_kind, self.media_width_mm,
            ))
        })
    }
}

pub(super) fn address_policy(settings: &LabelPrintingSettings) -> AddressPolicy {
    AddressPolicy {
        allow_loopback: settings.allow_loopback,
    }
}

/// Describes the media a status block reported, for error messages.
fn describe_loaded(status: &PrinterStatus) -> String {
    match status.media_kind.as_deref() {
        None => "no label stock".to_string(),
        Some(kind) if status.media_length_mm == 0 => {
            format!("{}mm {kind} stock", status.media_width_mm)
        }
        Some(kind) => format!(
            "{}x{}mm {kind} stock",
            status.media_width_mm, status.media_length_mm
        ),
    }
}

fn describe_expected(media: LabelPrinterMedia) -> String {
    match media.length_mm() {
        Some(length) => format!("{}x{length}mm {} stock", media.width_mm(), media.kind_str()),
        None => format!("{}mm {} stock", media.width_mm(), media.kind_str()),
    }
}

/// Refuses the job unless the printer is ready and holding the right stock.
pub(super) fn check_ready(
    status: &PrinterStatus,
    media: LabelPrinterMedia,
) -> Result<(), PrintError> {
    if !status.is_ready() {
        let faults: Vec<String> = status
            .faults
            .iter()
            .map(|fault| {
                serde_json::to_value(fault)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "unknown".into())
            })
            .collect();
        return Err(PrintError::NotReady(faults.join(", ")));
    }

    if !status.matches(media.spec()) {
        return Err(PrintError::MediaMismatch {
            loaded: describe_loaded(status),
            expected: describe_expected(media),
        });
    }

    Ok(())
}

/// A page as it arrives from the client, before validation.
pub(super) struct RequestedPage {
    pub(super) width_dots: u16,
    pub(super) height_dots: u16,
    pub(super) bitmap: Vec<u8>,
}

/// Validates every page against the loaded stock and expands the copy count.
///
/// Copies repeat each label consecutively rather than repeating the whole
/// batch, so printing two of each of ten assets yields pairs that can be
/// peeled off together.
pub(super) fn build_pages(
    media: LabelPrinterMedia,
    requested: Vec<RequestedPage>,
    copies: u32,
) -> Result<Vec<Page>, PrintError> {
    if requested.is_empty() {
        return Err(PrintError::Validation(
            "A print request must contain at least one label.".into(),
        ));
    }
    if copies == 0 || copies > MAX_COPIES {
        return Err(PrintError::Validation(format!(
            "Copies must be between 1 and {MAX_COPIES}."
        )));
    }

    let total = requested.len().saturating_mul(copies as usize);
    if total > MAX_LABELS_PER_REQUEST {
        return Err(PrintError::Validation(format!(
            "A print request may produce at most {MAX_LABELS_PER_REQUEST} labels; this one would produce {total}."
        )));
    }

    let spec = media.spec();
    let mut pages = Vec::with_capacity(total);
    for page in requested {
        // Continuous stock has no fixed length, so only die-cut labels are
        // checked against a length the media dictates.
        if spec.printable_length_dots != 0 && page.height_dots != spec.printable_length_dots {
            return Err(PrintError::Validation(format!(
                "Label is {} dots long but {} die-cut labels are {} dots.",
                page.height_dots,
                describe_expected(media),
                spec.printable_length_dots,
            )));
        }

        let built = Page::new(spec, page.width_dots, page.height_dots, page.bitmap)?;
        for _ in 0..copies {
            pages.push(built.clone());
        }
    }

    Ok(pages)
}

/// Encodes the job. Split out so the byte stream can be asserted in tests
/// without a socket.
pub(super) fn encode(
    media: LabelPrinterMedia,
    auto_cut: bool,
    pages: &[Page],
) -> Result<Vec<u8>, PrintError> {
    Ok(raster::encode_job(media.spec(), auto_cut, pages)?)
}

/// Upper bound on the bitmap bytes a single page may carry, used to reject
/// oversized payloads before they are decoded.
pub(super) fn max_page_bytes(media: LabelPrinterMedia) -> usize {
    raster::row_bytes(media.spec().printable_width_dots) * MAX_RASTER_LINES as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label_printing::media::MediaKind;
    use crate::label_printing::status::PrinterFault;

    fn die_cut() -> LabelPrinterMedia {
        LabelPrinterMedia::parse("die_cut", 62, Some(29)).expect("62x29 is supported")
    }

    fn continuous() -> LabelPrinterMedia {
        LabelPrinterMedia::parse("continuous", 62, None).expect("62mm is supported")
    }

    fn page(width: u16, height: u16) -> RequestedPage {
        RequestedPage {
            width_dots: width,
            height_dots: height,
            bitmap: vec![0; raster::row_bytes(width) * usize::from(height)],
        }
    }

    fn status_for(kind: MediaKind, width: u8, length: u8) -> PrinterStatus {
        let mut block = [0u8; 32];
        block[10] = width;
        block[11] = kind.print_information_code();
        block[17] = length;
        PrinterStatus::parse(&block).expect("block is well formed")
    }

    #[test]
    fn copies_repeat_each_label_consecutively() {
        let pages = build_pages(die_cut(), vec![page(696, 271), page(696, 271)], 3)
            .expect("pages are valid");
        assert_eq!(pages.len(), 6);
    }

    #[test]
    fn an_empty_request_is_rejected() {
        assert!(matches!(
            build_pages(die_cut(), vec![], 1),
            Err(PrintError::Validation(_))
        ));
    }

    #[test]
    fn copy_counts_are_bounded() {
        assert!(build_pages(die_cut(), vec![page(696, 271)], 0).is_err());
        assert!(build_pages(die_cut(), vec![page(696, 271)], MAX_COPIES).is_ok());
        assert!(build_pages(die_cut(), vec![page(696, 271)], MAX_COPIES + 1).is_err());
    }

    #[test]
    fn the_total_label_count_is_bounded() {
        let requested: Vec<_> = (0..30).map(|_| page(696, 271)).collect();
        let error = build_pages(die_cut(), requested, 20).unwrap_err();
        assert!(matches!(error, PrintError::Validation(message) if message.contains("600")));
    }

    #[test]
    fn die_cut_pages_must_match_the_label_length() {
        assert!(build_pages(die_cut(), vec![page(696, 271)], 1).is_ok());
        assert!(build_pages(die_cut(), vec![page(696, 300)], 1).is_err());
    }

    #[test]
    fn continuous_pages_may_be_any_length_within_range() {
        assert!(build_pages(continuous(), vec![page(696, 200)], 1).is_ok());
        assert!(build_pages(continuous(), vec![page(696, 900)], 1).is_ok());
        // Still bounded by what the printer accepts.
        assert!(build_pages(continuous(), vec![page(696, 100)], 1).is_err());
    }

    #[test]
    fn a_page_of_the_wrong_width_is_rejected() {
        assert!(build_pages(die_cut(), vec![page(306, 271)], 1).is_err());
    }

    #[test]
    fn a_ready_printer_with_matching_stock_passes() {
        let status = status_for(MediaKind::DieCut, 62, 29);
        assert!(check_ready(&status, die_cut()).is_ok());
    }

    #[test]
    fn mismatched_stock_is_refused_before_printing() {
        let status = status_for(MediaKind::DieCut, 62, 100);
        let error = check_ready(&status, die_cut()).unwrap_err();
        assert!(matches!(
            error,
            PrintError::MediaMismatch { ref loaded, ref expected }
                if loaded.contains("62x100") && expected.contains("62x29")
        ));
    }

    #[test]
    fn an_empty_printer_is_refused_with_a_readable_message() {
        let status = status_for(MediaKind::Continuous, 0, 0);
        let error = check_ready(&status, die_cut()).unwrap_err();
        assert!(matches!(error, PrintError::MediaMismatch { .. }));
    }

    #[test]
    fn a_faulted_printer_is_refused_and_names_the_faults() {
        let mut block = [0u8; 32];
        block[8] = 0b0000_0001; // no media
        block[9] = 0b0001_0000; // cover open
        block[10] = 62;
        block[11] = 0x0B;
        block[17] = 29;
        let status = PrinterStatus::parse(&block).expect("block is well formed");
        assert_eq!(
            status.faults,
            vec![PrinterFault::NoMedia, PrinterFault::CoverOpen]
        );

        let error = check_ready(&status, die_cut()).unwrap_err();
        assert!(matches!(error, PrintError::NotReady(ref message)
                if message.contains("no_media") && message.contains("cover_open")));
    }
}
