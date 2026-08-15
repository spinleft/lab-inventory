use crate::label_printing::media::{self, MediaKind, MediaSpec};

/// The label stock a printer is loaded with.
///
/// Kind, width and length only mean anything together — a die-cut size needs a
/// length, a continuous one must not have one, and the combination has to be a
/// size the raster layer knows how to place. Keeping them in one value object
/// means no caller can set two of the three and leave the printer describing
/// stock that cannot be printed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelPrinterMedia {
    spec: &'static MediaSpec,
}

impl LabelPrinterMedia {
    pub fn parse(kind: &str, width_mm: i32, length_mm: Option<i32>) -> Result<Self, String> {
        let Some(kind) = MediaKind::parse(kind) else {
            return Err(format!("{kind} is not a supported label media kind."));
        };

        let width = u8::try_from(width_mm)
            .map_err(|_| format!("{width_mm}mm is not a supported label width."))?;

        let length = match length_mm {
            Some(value) => Some(
                u8::try_from(value)
                    .map_err(|_| format!("{value}mm is not a supported label length."))?,
            ),
            None => None,
        };

        match (kind, length) {
            (MediaKind::Continuous, Some(_)) => {
                return Err("Continuous label stock has no fixed length.".into());
            }
            (MediaKind::DieCut, None) => {
                return Err("Die-cut label stock requires a length.".into());
            }
            _ => {}
        }

        let spec = media::lookup(kind, width, length).ok_or_else(|| match length {
            Some(length) => format!("{width}x{length}mm die-cut labels are not supported."),
            None => format!("{width}mm continuous labels are not supported."),
        })?;

        Ok(Self { spec })
    }

    pub fn spec(&self) -> &'static MediaSpec {
        self.spec
    }

    pub fn kind(&self) -> MediaKind {
        self.spec.kind
    }

    pub fn kind_str(&self) -> &'static str {
        self.spec.kind.as_str()
    }

    pub fn width_mm(&self) -> u8 {
        self.spec.width_mm
    }

    /// `None` for continuous stock, matching how the column is stored.
    pub fn length_mm(&self) -> Option<u8> {
        match self.spec.kind {
            MediaKind::Continuous => None,
            MediaKind::DieCut => Some(self.spec.length_mm),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LabelPrinterMedia;
    use claims::{assert_err, assert_ok};

    #[test]
    fn supported_die_cut_stock_is_parsed() {
        let media = LabelPrinterMedia::parse("die_cut", 62, Some(29)).expect("62x29 is supported");
        assert_eq!(media.width_mm(), 62);
        assert_eq!(media.length_mm(), Some(29));
        assert_eq!(media.spec().printable_width_dots, 696);
    }

    #[test]
    fn supported_continuous_stock_is_parsed() {
        let media = LabelPrinterMedia::parse("continuous", 62, None).expect("62mm is supported");
        assert_eq!(media.length_mm(), None);
        assert_eq!(media.spec().printable_width_dots, 696);
    }

    #[test]
    fn unknown_kinds_are_rejected() {
        assert_err!(LabelPrinterMedia::parse("roll", 62, None));
    }

    #[test]
    fn continuous_stock_may_not_carry_a_length() {
        assert_err!(LabelPrinterMedia::parse("continuous", 62, Some(29)));
    }

    #[test]
    fn die_cut_stock_requires_a_length() {
        assert_err!(LabelPrinterMedia::parse("die_cut", 62, None));
    }

    #[test]
    fn unsupported_sizes_are_rejected() {
        assert_err!(LabelPrinterMedia::parse("die_cut", 62, Some(30)));
        assert_err!(LabelPrinterMedia::parse("continuous", 45, None));
    }

    #[test]
    fn out_of_range_dimensions_are_rejected_rather_than_wrapping() {
        assert_err!(LabelPrinterMedia::parse("continuous", 300, None));
        assert_err!(LabelPrinterMedia::parse("continuous", -1, None));
        assert_err!(LabelPrinterMedia::parse("die_cut", 62, Some(300)));
    }

    #[test]
    fn every_supported_size_round_trips_through_parse() {
        for spec in crate::label_printing::media::SUPPORTED_MEDIA {
            let length = match spec.kind {
                crate::label_printing::media::MediaKind::Continuous => None,
                crate::label_printing::media::MediaKind::DieCut => Some(i32::from(spec.length_mm)),
            };
            assert_ok!(LabelPrinterMedia::parse(
                spec.kind.as_str(),
                i32::from(spec.width_mm),
                length,
            ));
        }
    }
}
