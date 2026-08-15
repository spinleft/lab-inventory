//! Parsing of the 32-byte status block a printer returns for `ESC i S`.
//!
//! The block is what lets a print be refused *before* it runs: it reports the
//! media actually loaded, so a job laid out for one label size never gets
//! printed onto a different roll.
use super::media::{MediaKind, MediaSpec};
use serde::Serialize;

/// Length of the status block the printer sends back.
pub const STATUS_BLOCK_LEN: usize = 32;

const OFFSET_ERROR_1: usize = 8;
const OFFSET_ERROR_2: usize = 9;
const OFFSET_MEDIA_WIDTH: usize = 10;
const OFFSET_MEDIA_TYPE: usize = 11;
const OFFSET_MEDIA_LENGTH: usize = 17;
// Bytes 18 and 19 carry the status and phase types. They describe *why* the
// printer replied, which matters for streaming job progress but not for the
// pre-flight check this module exists to serve, so they are left unread.

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum StatusError {
    #[error("Printer returned {0} bytes, expected at least {STATUS_BLOCK_LEN}")]
    TooShort(usize),
}

/// A condition the printer is reporting.
///
/// Serialized as a stable code so the client can present its own wording.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterFault {
    NoMedia,
    EndOfMedia,
    CutterJam,
    MainUnitInUse,
    PrinterTurnedOff,
    HighVoltageAdapter,
    FanError,
    ReplaceMedia,
    ExpansionBufferFull,
    CommunicationError,
    CommunicationBufferFull,
    CoverOpen,
    CancelKey,
    CannotFeedMedia,
    SystemError,
}

/// Bit position → fault, for the first error byte. Bit 3 is unused.
const ERROR_1_FAULTS: [(u8, PrinterFault); 7] = [
    (0, PrinterFault::NoMedia),
    (1, PrinterFault::EndOfMedia),
    (2, PrinterFault::CutterJam),
    (4, PrinterFault::MainUnitInUse),
    (5, PrinterFault::PrinterTurnedOff),
    (6, PrinterFault::HighVoltageAdapter),
    (7, PrinterFault::FanError),
];

/// Bit position → fault, for the second error byte.
const ERROR_2_FAULTS: [(u8, PrinterFault); 8] = [
    (0, PrinterFault::ReplaceMedia),
    (1, PrinterFault::ExpansionBufferFull),
    (2, PrinterFault::CommunicationError),
    (3, PrinterFault::CommunicationBufferFull),
    (4, PrinterFault::CoverOpen),
    (5, PrinterFault::CancelKey),
    (6, PrinterFault::CannotFeedMedia),
    (7, PrinterFault::SystemError),
];

/// What the printer reported about itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrinterStatus {
    /// `None` when no media is loaded or the code is unrecognised.
    pub media_kind: Option<String>,
    pub media_width_mm: u8,
    /// Zero for continuous stock.
    pub media_length_mm: u8,
    pub faults: Vec<PrinterFault>,
}

impl PrinterStatus {
    pub fn parse(block: &[u8]) -> Result<Self, StatusError> {
        if block.len() < STATUS_BLOCK_LEN {
            return Err(StatusError::TooShort(block.len()));
        }

        let media_kind = match block[OFFSET_MEDIA_TYPE] {
            0x0A => Some(MediaKind::Continuous),
            0x0B => Some(MediaKind::DieCut),
            _ => None,
        };

        let mut faults = Vec::new();
        collect_faults(block[OFFSET_ERROR_1], &ERROR_1_FAULTS, &mut faults);
        collect_faults(block[OFFSET_ERROR_2], &ERROR_2_FAULTS, &mut faults);

        Ok(Self {
            media_kind: media_kind.map(|kind| kind.as_str().to_owned()),
            media_width_mm: block[OFFSET_MEDIA_WIDTH],
            media_length_mm: block[OFFSET_MEDIA_LENGTH],
            faults,
        })
    }

    /// Whether the loaded media is the one a job was laid out for.
    pub fn matches(&self, spec: &MediaSpec) -> bool {
        if self.media_kind.as_deref() != Some(spec.kind.as_str()) {
            return false;
        }
        if self.media_width_mm != spec.width_mm {
            return false;
        }
        match spec.kind {
            // Continuous stock has no length to compare.
            MediaKind::Continuous => true,
            MediaKind::DieCut => self.media_length_mm == spec.length_mm,
        }
    }

    /// Faults that should stop a job from being sent.
    pub fn is_ready(&self) -> bool {
        self.faults.is_empty()
    }
}

fn collect_faults(byte: u8, table: &[(u8, PrinterFault)], out: &mut Vec<PrinterFault>) {
    for &(bit, fault) in table {
        if byte & (1 << bit) != 0 {
            out.push(fault);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label_printing::media;

    /// A block reporting 62x29 die-cut labels loaded and no faults.
    fn healthy_block() -> [u8; STATUS_BLOCK_LEN] {
        let mut block = [0u8; STATUS_BLOCK_LEN];
        block[0] = 0x80;
        block[1] = 0x20;
        block[2] = 0x42;
        block[3] = 0x30;
        block[OFFSET_MEDIA_WIDTH] = 62;
        block[OFFSET_MEDIA_TYPE] = 0x0B;
        block[OFFSET_MEDIA_LENGTH] = 29;
        block
    }

    #[test]
    fn parses_loaded_die_cut_media() {
        let status = PrinterStatus::parse(&healthy_block()).expect("block is well formed");
        assert_eq!(status.media_kind.as_deref(), Some("die_cut"));
        assert_eq!(status.media_width_mm, 62);
        assert_eq!(status.media_length_mm, 29);
        assert!(status.is_ready());
    }

    #[test]
    fn rejects_a_truncated_block() {
        assert_eq!(
            PrinterStatus::parse(&[0u8; 10]).unwrap_err(),
            StatusError::TooShort(10)
        );
    }

    #[test]
    fn reports_no_media_as_an_unknown_kind() {
        let mut block = healthy_block();
        block[OFFSET_MEDIA_TYPE] = 0x00;
        let status = PrinterStatus::parse(&block).expect("block is well formed");
        assert_eq!(status.media_kind, None);
    }

    #[test]
    fn collects_faults_from_both_error_bytes() {
        let mut block = healthy_block();
        block[OFFSET_ERROR_1] = 0b0000_0001; // no media
        block[OFFSET_ERROR_2] = 0b0001_0000; // cover open
        let status = PrinterStatus::parse(&block).expect("block is well formed");
        assert_eq!(
            status.faults,
            vec![PrinterFault::NoMedia, PrinterFault::CoverOpen]
        );
        assert!(!status.is_ready());
    }

    #[test]
    fn ignores_the_unused_bit_of_the_first_error_byte() {
        let mut block = healthy_block();
        block[OFFSET_ERROR_1] = 0b0000_1000;
        let status = PrinterStatus::parse(&block).expect("block is well formed");
        assert!(status.faults.is_empty());
    }

    #[test]
    fn matches_the_media_a_job_was_laid_out_for() {
        let status = PrinterStatus::parse(&healthy_block()).expect("block is well formed");
        let die_cut = media::lookup(MediaKind::DieCut, 62, Some(29)).expect("62x29 is supported");
        assert!(status.matches(die_cut));

        let wrong_length =
            media::lookup(MediaKind::DieCut, 62, Some(100)).expect("62x100 is supported");
        assert!(!status.matches(wrong_length));

        let wrong_kind =
            media::lookup(MediaKind::Continuous, 62, None).expect("62mm is supported");
        assert!(!status.matches(wrong_kind));
    }

    #[test]
    fn continuous_media_matches_regardless_of_reported_length() {
        let mut block = healthy_block();
        block[OFFSET_MEDIA_TYPE] = 0x0A;
        block[OFFSET_MEDIA_LENGTH] = 0;
        let status = PrinterStatus::parse(&block).expect("block is well formed");
        let spec = media::lookup(MediaKind::Continuous, 62, None).expect("62mm is supported");
        assert!(status.matches(spec));
    }
}
