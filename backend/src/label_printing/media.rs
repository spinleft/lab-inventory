//! Physical label stock specifications.
//!
//! The printer always shifts out a full [`PINS`]-dot raster row regardless of
//! how wide the loaded tape actually is, so every label size carries the two
//! numbers needed to place a bitmap inside that row: how many dots of it are
//! printable, and how far the printable area sits from the right edge.
//!
//! [`PINS`]: super::PINS
use super::PINS;

/// How the label stock is separated into individual labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    /// One long roll cut to length by the printer.
    Continuous,
    /// Pre-cut labels of a fixed length.
    DieCut,
}

impl MediaKind {
    /// The value the `ESC i z` print-information command uses for this kind.
    pub fn print_information_code(self) -> u8 {
        match self {
            Self::Continuous => 0x0A,
            Self::DieCut => 0x0B,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Continuous => "continuous",
            Self::DieCut => "die_cut",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "continuous" => Some(Self::Continuous),
            "die_cut" => Some(Self::DieCut),
            _ => None,
        }
    }
}

/// A supported label size.
///
/// `length_mm` and `printable_length_dots` are zero for continuous stock, where
/// the length is whatever the caller rasterises rather than a property of the
/// media.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaSpec {
    pub kind: MediaKind,
    pub width_mm: u8,
    pub length_mm: u8,
    pub total_width_dots: u16,
    pub printable_width_dots: u16,
    pub printable_length_dots: u16,
    /// Dots between the printable area and the right edge of the print head.
    pub right_margin_dots: u16,
    /// Extra feed the printer needs before the next label on continuous stock.
    pub feed_margin_dots: u16,
}

impl MediaSpec {
    /// Where the bitmap's left edge lands inside the full raster row.
    ///
    /// The bitmap is right-aligned against the printable area, which itself
    /// sits `right_margin_dots` from the right edge of the head.
    pub fn left_offset_dots(&self, bitmap_width_dots: u16) -> Option<u16> {
        PINS.checked_sub(bitmap_width_dots)?
            .checked_sub(self.right_margin_dots)
    }

    /// The margin the `ESC i d` command should be given for this stock.
    ///
    /// Die-cut labels are already separated, so they need no inter-label feed.
    pub fn feed_margin_for_print(&self) -> u16 {
        match self.kind {
            MediaKind::Continuous => self.feed_margin_dots,
            MediaKind::DieCut => 0,
        }
    }
}

const fn continuous(
    width_mm: u8,
    total_width_dots: u16,
    printable_width_dots: u16,
    right_margin_dots: u16,
) -> MediaSpec {
    MediaSpec {
        kind: MediaKind::Continuous,
        width_mm,
        length_mm: 0,
        total_width_dots,
        printable_width_dots,
        printable_length_dots: 0,
        right_margin_dots,
        feed_margin_dots: 35,
    }
}

const fn die_cut(
    width_mm: u8,
    length_mm: u8,
    total_width_dots: u16,
    printable_width_dots: u16,
    printable_length_dots: u16,
    right_margin_dots: u16,
) -> MediaSpec {
    MediaSpec {
        kind: MediaKind::DieCut,
        width_mm,
        length_mm,
        total_width_dots,
        printable_width_dots,
        printable_length_dots,
        right_margin_dots,
        feed_margin_dots: 0,
    }
}

/// Every label size this server knows how to lay out.
///
/// Adding a size is a one-line change here; the numbers come from Brother's
/// raster command reference.
pub const SUPPORTED_MEDIA: &[MediaSpec] = &[
    continuous(29, 342, 306, 6),
    continuous(38, 449, 413, 12),
    continuous(50, 590, 554, 12),
    continuous(62, 732, 696, 12),
    die_cut(17, 54, 201, 165, 566, 0),
    die_cut(23, 23, 272, 202, 202, 42),
    die_cut(29, 90, 342, 306, 991, 6),
    die_cut(62, 29, 732, 696, 271, 12),
    die_cut(62, 100, 732, 696, 1109, 12),
];

/// Finds the spec for a configured printer's media.
///
/// `length_mm` is ignored for continuous stock and required for die-cut.
pub fn lookup(kind: MediaKind, width_mm: u8, length_mm: Option<u8>) -> Option<&'static MediaSpec> {
    SUPPORTED_MEDIA.iter().find(|spec| {
        spec.kind == kind
            && spec.width_mm == width_mm
            && match kind {
                MediaKind::Continuous => true,
                MediaKind::DieCut => length_mm == Some(spec.length_mm),
            }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_lookup_ignores_length() {
        let spec = lookup(MediaKind::Continuous, 62, None).expect("62mm continuous is supported");
        assert_eq!(spec.printable_width_dots, 696);
        assert_eq!(spec.right_margin_dots, 12);
        assert_eq!(spec.feed_margin_for_print(), 35);
    }

    #[test]
    fn die_cut_lookup_requires_matching_length() {
        assert!(lookup(MediaKind::DieCut, 62, Some(29)).is_some());
        assert!(lookup(MediaKind::DieCut, 62, Some(30)).is_none());
        assert!(lookup(MediaKind::DieCut, 62, None).is_none());
    }

    #[test]
    fn die_cut_needs_no_feed_margin() {
        let spec = lookup(MediaKind::DieCut, 62, Some(29)).expect("62x29 is supported");
        assert_eq!(spec.printable_length_dots, 271);
        assert_eq!(spec.feed_margin_for_print(), 0);
    }

    #[test]
    fn left_offset_right_aligns_against_the_printable_area() {
        // 720 - 696 - 12 == 12
        let spec = lookup(MediaKind::Continuous, 62, None).expect("62mm continuous is supported");
        assert_eq!(spec.left_offset_dots(696), Some(12));

        // Narrow stock leaves a much larger gap on the left.
        let narrow = lookup(MediaKind::Continuous, 29, None).expect("29mm continuous is supported");
        assert_eq!(narrow.left_offset_dots(306), Some(408));

        let square = lookup(MediaKind::DieCut, 23, Some(23)).expect("23x23 is supported");
        assert_eq!(square.left_offset_dots(202), Some(476));
    }

    #[test]
    fn left_offset_rejects_a_bitmap_that_cannot_fit() {
        let spec = lookup(MediaKind::Continuous, 62, None).expect("62mm continuous is supported");
        assert_eq!(spec.left_offset_dots(PINS), None);
        assert_eq!(spec.left_offset_dots(PINS - 11), None);
    }

    #[test]
    fn every_spec_places_its_printable_area_inside_the_head() {
        for spec in SUPPORTED_MEDIA {
            assert!(
                spec.left_offset_dots(spec.printable_width_dots).is_some(),
                "{}mm {} does not fit within {PINS} dots",
                spec.width_mm,
                spec.kind.as_str(),
            );
            assert!(spec.printable_width_dots <= spec.total_width_dots);
        }
    }

    #[test]
    fn media_kind_round_trips_through_its_string_form() {
        for kind in [MediaKind::Continuous, MediaKind::DieCut] {
            assert_eq!(MediaKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(MediaKind::parse("roll"), None);
    }
}
