//! Brother raster command encoding.
//!
//! Turns one or more 1-bit page bitmaps into the byte stream a QL-series
//! printer expects on its raw TCP port. Everything here is pure: the caller
//! supplies the bitmaps and the media spec, and gets bytes back.
//!
//! Bitmap convention, matching what the printer wants on the wire: rows are
//! packed MSB-first, and **a set bit means a black dot**. The leftmost dot of a
//! row is bit 7 of byte 0.
use super::media::MediaSpec;
use super::{BYTES_PER_ROW, INVALIDATE_BYTES, MAX_RASTER_LINES, MIN_RASTER_LINES};

const ESC: u8 = 0x1B;

/// A single label's bitmap.
#[derive(Clone, Debug)]
pub struct Page {
    width_dots: u16,
    height_dots: u16,
    bitmap: Vec<u8>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum RasterError {
    #[error("A print job must contain at least one page")]
    NoPages,
    #[error("Label bitmap is {width} dots wide but the loaded media prints {printable} dots")]
    WidthMismatch { width: u16, printable: u16 },
    #[error(
        "Label bitmap is {height} dots long, outside the printable range of {MIN_RASTER_LINES}-{MAX_RASTER_LINES} dots"
    )]
    LengthOutOfRange { height: u16 },
    #[error("Label bitmap is {actual} bytes but {width}x{height} dots needs {expected} bytes")]
    BitmapSizeMismatch {
        width: u16,
        height: u16,
        expected: usize,
        actual: usize,
    },
}

impl Page {
    /// Validates a bitmap against the media it is going to be printed on.
    pub fn new(
        media: &MediaSpec,
        width_dots: u16,
        height_dots: u16,
        bitmap: Vec<u8>,
    ) -> Result<Self, RasterError> {
        if width_dots != media.printable_width_dots {
            return Err(RasterError::WidthMismatch {
                width: width_dots,
                printable: media.printable_width_dots,
            });
        }

        let height = u32::from(height_dots);
        if !(MIN_RASTER_LINES..=MAX_RASTER_LINES).contains(&height) {
            return Err(RasterError::LengthOutOfRange {
                height: height_dots,
            });
        }

        let expected = row_bytes(width_dots) * usize::from(height_dots);
        if bitmap.len() != expected {
            return Err(RasterError::BitmapSizeMismatch {
                width: width_dots,
                height: height_dots,
                expected,
                actual: bitmap.len(),
            });
        }

        Ok(Self {
            width_dots,
            height_dots,
            bitmap,
        })
    }

    fn row(&self, index: u16) -> &[u8] {
        let stride = row_bytes(self.width_dots);
        let start = stride * usize::from(index);
        &self.bitmap[start..start + stride]
    }
}

/// Bytes needed to hold one row of `width_dots` pixels, one bit per pixel.
pub fn row_bytes(width_dots: u16) -> usize {
    usize::from(width_dots).div_ceil(8)
}

/// Encodes a complete print job.
///
/// The session preamble is emitted once; every page then carries its own print
/// information, mode and margin commands. Only the final page ends with the
/// feed-and-cut byte, so a batch of labels reaches the printer as one job
/// rather than as N jobs that each eject the tape.
pub fn encode_job(
    media: &MediaSpec,
    auto_cut: bool,
    pages: &[Page],
) -> Result<Vec<u8>, RasterError> {
    if pages.is_empty() {
        return Err(RasterError::NoPages);
    }

    let mut out = Vec::new();
    push_preamble(&mut out);

    let last_index = pages.len() - 1;
    for (index, page) in pages.iter().enumerate() {
        push_print_information(&mut out, media, page, index);
        push_mode_commands(&mut out, auto_cut);
        push_margins(&mut out, media);
        push_compression(&mut out);
        push_raster_rows(&mut out, media, page);
        out.push(if index == last_index { 0x1A } else { 0x0C });
    }

    Ok(out)
}

/// Resets the printer and puts it into raster mode.
fn push_preamble(out: &mut Vec<u8>) {
    // Flushes any half-finished job left in the printer's buffer.
    out.extend(std::iter::repeat_n(0x00, INVALIDATE_BYTES));
    out.extend_from_slice(&[ESC, b'@']);
    out.extend_from_slice(&[ESC, b'i', b'a', 0x01]);
    out.extend_from_slice(&status_request());
}

/// `ESC i S` — asks the printer to report back a 32-byte status block.
pub fn status_request() -> [u8; 3] {
    [ESC, b'i', b'S']
}

/// `ESC i z` — describes the media and the length of the page that follows.
fn push_print_information(out: &mut Vec<u8>, media: &MediaSpec, page: &Page, page_index: usize) {
    // 0x80 keeps printer recovery on; the rest mark type, width, length and
    // print-quality as carrying meaningful values.
    const VALID_FLAGS: u8 = 0x80 | 0x02 | 0x04 | 0x08 | 0x40;

    out.extend_from_slice(&[ESC, b'i', b'z']);
    out.push(VALID_FLAGS);
    out.push(media.kind.print_information_code());
    out.push(media.width_mm);
    // Continuous stock has no fixed length, and reports zero here.
    out.push(media.length_mm);
    out.extend_from_slice(&u32::from(page.height_dots).to_le_bytes());
    out.push(if page_index == 0 { 0 } else { 1 });
    out.push(0x00);
}

/// `ESC i M` / `ESC i A` / `ESC i K` — cutting behaviour.
fn push_mode_commands(out: &mut Vec<u8>, auto_cut: bool) {
    out.extend_from_slice(&[ESC, b'i', b'M']);
    out.push(u8::from(auto_cut) << 6);

    out.extend_from_slice(&[ESC, b'i', b'A']);
    out.push(1);

    out.extend_from_slice(&[ESC, b'i', b'K']);
    // Bit 3 cuts after the last label. Bit 0 (two-colour) and bit 6 (600 dpi)
    // stay off.
    out.push(u8::from(auto_cut) << 3);
}

/// `ESC i d` — feed margin, which only continuous stock needs.
fn push_margins(out: &mut Vec<u8>, media: &MediaSpec) {
    out.extend_from_slice(&[ESC, b'i', b'd']);
    out.extend_from_slice(&media.feed_margin_for_print().to_le_bytes());
}

/// `M` — enable TIFF/PackBits compression for the raster rows.
fn push_compression(out: &mut Vec<u8>) {
    out.extend_from_slice(&[b'M', 0x02]);
}

/// `g` — one compressed raster row per print-head line.
fn push_raster_rows(out: &mut Vec<u8>, media: &MediaSpec, page: &Page) {
    // Validated when the page was built, so the offset is always available.
    let left_offset = media.left_offset_dots(page.width_dots).unwrap_or(0);

    let mut line = [0u8; BYTES_PER_ROW];
    for index in 0..page.height_dots {
        line.fill(0);
        blit_row(&mut line, page.row(index), page.width_dots, left_offset);

        let compressed = pack_bits(&line);
        out.extend_from_slice(&[b'g', 0x00, compressed.len() as u8]);
        out.extend_from_slice(&compressed);
    }
}

/// Copies a page row into the full-width print-head line at `left_offset`.
fn blit_row(line: &mut [u8; BYTES_PER_ROW], row: &[u8], width_dots: u16, left_offset: u16) {
    for x in 0..width_dots {
        let source = usize::from(x);
        if row[source / 8] & (0x80 >> (source % 8)) == 0 {
            continue;
        }
        let target = usize::from(left_offset) + source;
        line[target / 8] |= 0x80 >> (target % 8);
    }
}

/// TIFF/PackBits run-length encoding.
///
/// Label bitmaps are mostly blank, so this typically shrinks a 90-byte line to
/// a couple of bytes.
fn pack_bits(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index = 0;

    while index < data.len() {
        let run = run_length(data, index);
        if run >= 2 {
            let take = run.min(128);
            out.push((257 - take) as u8);
            out.push(data[index]);
            index += take;
            continue;
        }

        let start = index;
        while index < data.len() && index - start < 128 && run_length(data, index) < 2 {
            index += 1;
        }
        out.push((index - start - 1) as u8);
        out.extend_from_slice(&data[start..index]);
    }

    out
}

/// How many times the byte at `index` repeats consecutively.
fn run_length(data: &[u8], index: usize) -> usize {
    let value = data[index];
    data[index..].iter().take_while(|&&b| b == value).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label_printing::media::{self, MediaKind};

    /// The inverse of [`pack_bits`], so round-trips can be asserted.
    fn unpack_bits(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut index = 0;
        while index < data.len() {
            let control = data[index];
            index += 1;
            match control {
                0..=127 => {
                    let count = usize::from(control) + 1;
                    out.extend_from_slice(&data[index..index + count]);
                    index += count;
                }
                128 => {}
                _ => {
                    let count = 257 - usize::from(control);
                    out.extend(std::iter::repeat_n(data[index], count));
                    index += 1;
                }
            }
        }
        out
    }

    fn die_cut_62x29() -> &'static MediaSpec {
        media::lookup(MediaKind::DieCut, 62, Some(29)).expect("62x29 is supported")
    }

    fn blank_page(media: &MediaSpec, height: u16) -> Page {
        let bitmap = vec![0u8; row_bytes(media.printable_width_dots) * usize::from(height)];
        Page::new(media, media.printable_width_dots, height, bitmap).expect("page is well formed")
    }

    #[test]
    fn pack_bits_round_trips_a_blank_line() {
        let line = [0u8; BYTES_PER_ROW];
        let packed = pack_bits(&line);
        // 90 identical bytes collapse to a single repeat run: 257 - 90 == 167.
        assert_eq!(packed, vec![167, 0x00]);
        assert_eq!(unpack_bits(&packed), line);
    }

    #[test]
    fn pack_bits_round_trips_mixed_content() {
        let data = b"aaabcdeeeeeeffgh".to_vec();
        assert_eq!(unpack_bits(&pack_bits(&data)), data);
    }

    #[test]
    fn pack_bits_round_trips_incompressible_content() {
        let data: Vec<u8> = (0..=255).collect();
        let packed = pack_bits(&data);
        assert_eq!(unpack_bits(&packed), data);
    }

    #[test]
    fn pack_bits_splits_runs_longer_than_the_control_byte_allows() {
        let data = vec![0x5Au8; 300];
        let packed = pack_bits(&data);
        assert_eq!(unpack_bits(&packed), data);
        // 300 = 128 + 128 + 44, so three repeat runs of two bytes each.
        assert_eq!(packed.len(), 6);
    }

    #[test]
    fn pack_bits_round_trips_every_line_of_a_realistic_bitmap() {
        // A diagonal stripe gives every row a different bit pattern.
        let media = die_cut_62x29();
        let stride = row_bytes(media.printable_width_dots);
        for row in 0..media.printable_length_dots {
            let mut line = vec![0u8; stride];
            let x = usize::from(row) % usize::from(media.printable_width_dots);
            line[x / 8] |= 0x80 >> (x % 8);
            assert_eq!(unpack_bits(&pack_bits(&line)), line);
        }
    }

    #[test]
    fn blit_places_the_leftmost_dot_at_the_media_offset() {
        let media = die_cut_62x29();
        let mut row = vec![0u8; row_bytes(media.printable_width_dots)];
        row[0] = 0x80; // leftmost dot of the label

        let mut line = [0u8; BYTES_PER_ROW];
        let offset = media
            .left_offset_dots(media.printable_width_dots)
            .expect("62x29 fits");
        assert_eq!(offset, 12);
        blit_row(&mut line, &row, media.printable_width_dots, offset);

        // Dot 12 is bit 4 of byte 1.
        assert_eq!(line[0], 0x00);
        assert_eq!(line[1], 0x08);
    }

    #[test]
    fn blit_places_the_rightmost_dot_before_the_right_margin() {
        let media = die_cut_62x29();
        let stride = row_bytes(media.printable_width_dots);
        let mut row = vec![0u8; stride];
        let last = usize::from(media.printable_width_dots) - 1;
        row[last / 8] |= 0x80 >> (last % 8);

        let mut line = [0u8; BYTES_PER_ROW];
        blit_row(&mut line, &row, media.printable_width_dots, 12);

        // 12 + 695 = 707, which is bit 4 of byte 88; the 12-dot right margin
        // leaves byte 89 untouched.
        assert_eq!(line[88], 0x10);
        assert_eq!(line[89], 0x00);
    }

    #[test]
    fn page_rejects_a_width_the_media_cannot_print() {
        let media = die_cut_62x29();
        let error = Page::new(media, 300, 271, vec![0; 38 * 271]).unwrap_err();
        assert_eq!(
            error,
            RasterError::WidthMismatch {
                width: 300,
                printable: 696,
            }
        );
    }

    #[test]
    fn page_rejects_a_bitmap_of_the_wrong_size() {
        let media = die_cut_62x29();
        let error = Page::new(media, 696, 271, vec![0; 10]).unwrap_err();
        assert_eq!(
            error,
            RasterError::BitmapSizeMismatch {
                width: 696,
                height: 271,
                expected: 87 * 271,
                actual: 10,
            }
        );
    }

    #[test]
    fn page_rejects_lengths_outside_the_printable_range() {
        let media = die_cut_62x29();
        let stride = row_bytes(media.printable_width_dots);

        let too_short = MIN_RASTER_LINES as u16 - 1;
        assert_eq!(
            Page::new(
                media,
                696,
                too_short,
                vec![0; stride * usize::from(too_short)]
            )
            .unwrap_err(),
            RasterError::LengthOutOfRange { height: too_short }
        );

        // Anything at or above the ceiling is refused; 11811 itself is allowed.
        assert!(Page::new(media, 696, 11811, vec![0; stride * 11811]).is_ok());
    }

    #[test]
    fn job_rejects_an_empty_page_list() {
        assert_eq!(
            encode_job(die_cut_62x29(), true, &[]).unwrap_err(),
            RasterError::NoPages
        );
    }

    #[test]
    fn job_emits_the_preamble_once() {
        let media = die_cut_62x29();
        let pages = [blank_page(media, 271), blank_page(media, 271)];
        let job = encode_job(media, true, &pages).expect("job encodes");

        assert_eq!(&job[..INVALIDATE_BYTES], &vec![0u8; INVALIDATE_BYTES][..]);
        assert_eq!(
            &job[INVALIDATE_BYTES..INVALIDATE_BYTES + 9],
            &[ESC, b'@', ESC, b'i', b'a', 0x01, ESC, b'i', b'S']
        );
        // A second reset would show up as another invalidate run.
        assert_eq!(
            job.windows(INVALIDATE_BYTES)
                .filter(|w| w.iter().all(|&b| b == 0))
                .count(),
            1
        );
    }

    #[test]
    fn job_terminates_only_the_final_page_with_a_feed() {
        let media = die_cut_62x29();
        let pages = [
            blank_page(media, 271),
            blank_page(media, 271),
            blank_page(media, 271),
        ];
        let job = encode_job(media, true, &pages).expect("job encodes");

        assert_eq!(job.iter().filter(|&&b| b == 0x0C).count(), 2);
        assert_eq!(job.last(), Some(&0x1A));
    }

    #[test]
    fn print_information_describes_the_media_and_page() {
        let media = die_cut_62x29();
        let pages = [blank_page(media, 271)];
        let job = encode_job(media, true, &pages).expect("job encodes");

        let start = job
            .windows(3)
            .position(|w| w == [ESC, b'i', b'z'])
            .expect("print information is emitted");
        assert_eq!(
            &job[start..start + 13],
            &[
                ESC, b'i', b'z', //
                0xCE, // recovery | type | width | length | quality
                0x0B, // die-cut
                62,   // width mm
                29,   // length mm
                0x0F, 0x01, 0x00, 0x00, // 271 raster lines, little endian
                0x00, // first page
                0x00,
            ]
        );
    }

    #[test]
    fn continuous_media_reports_zero_length_and_a_feed_margin() {
        let media = media::lookup(MediaKind::Continuous, 62, None).expect("62mm is supported");
        let pages = [blank_page(media, 400)];
        let job = encode_job(media, true, &pages).expect("job encodes");

        let start = job
            .windows(3)
            .position(|w| w == [ESC, b'i', b'z'])
            .expect("print information is emitted");
        assert_eq!(job[start + 4], 0x0A, "continuous media type");
        assert_eq!(job[start + 6], 0, "continuous stock reports no length");

        let margins = job
            .windows(3)
            .position(|w| w == [ESC, b'i', b'd'])
            .expect("margins are emitted");
        assert_eq!(&job[margins + 3..margins + 5], &35u16.to_le_bytes());
    }

    #[test]
    fn die_cut_media_asks_for_no_feed_margin() {
        let media = die_cut_62x29();
        let pages = [blank_page(media, 271)];
        let job = encode_job(media, true, &pages).expect("job encodes");

        let margins = job
            .windows(3)
            .position(|w| w == [ESC, b'i', b'd'])
            .expect("margins are emitted");
        assert_eq!(&job[margins + 3..margins + 5], &0u16.to_le_bytes());
    }

    #[test]
    fn later_pages_are_marked_as_continuations() {
        let media = die_cut_62x29();
        let pages = [blank_page(media, 271), blank_page(media, 271)];
        let job = encode_job(media, true, &pages).expect("job encodes");

        let page_flags: Vec<u8> = job
            .windows(3)
            .enumerate()
            .filter(|(_, w)| *w == [ESC, b'i', b'z'])
            .map(|(start, _)| job[start + 11])
            .collect();
        assert_eq!(page_flags, vec![0, 1]);
    }

    #[test]
    fn disabling_auto_cut_clears_both_cut_flags() {
        let media = die_cut_62x29();
        let pages = [blank_page(media, 271)];

        let cut = encode_job(media, true, &pages).expect("job encodes");
        let various = cut
            .windows(3)
            .position(|w| w == [ESC, b'i', b'M'])
            .expect("various mode is emitted");
        let expanded = cut
            .windows(3)
            .position(|w| w == [ESC, b'i', b'K'])
            .expect("expanded mode is emitted");
        assert_eq!(cut[various + 3], 0x40);
        assert_eq!(cut[expanded + 3], 0x08);

        let uncut = encode_job(media, false, &pages).expect("job encodes");
        assert_eq!(uncut[various + 3], 0x00);
        assert_eq!(uncut[expanded + 3], 0x00);
    }

    #[test]
    fn every_raster_row_decompresses_to_a_full_print_head_line() {
        let media = die_cut_62x29();
        let stride = row_bytes(media.printable_width_dots);
        // Put one dot per row at a moving position so no two rows are alike.
        let mut bitmap = vec![0u8; stride * 271];
        for row in 0..271usize {
            let x = row % usize::from(media.printable_width_dots);
            bitmap[row * stride + x / 8] |= 0x80 >> (x % 8);
        }
        let page =
            Page::new(media, media.printable_width_dots, 271, bitmap).expect("page is valid");
        let job = encode_job(media, true, &[page]).expect("job encodes");

        let mut rows = 0;
        let mut index = 0;
        while index + 3 <= job.len() {
            if job[index] == b'g' && job[index + 1] == 0x00 {
                let length = usize::from(job[index + 2]);
                let decoded = unpack_bits(&job[index + 3..index + 3 + length]);
                assert_eq!(
                    decoded.len(),
                    BYTES_PER_ROW,
                    "row {rows} is the wrong width"
                );
                assert_eq!(
                    decoded.iter().filter(|b| b.count_ones() > 0).count(),
                    1,
                    "row {rows} should carry exactly one dot"
                );
                rows += 1;
                index += 3 + length;
                continue;
            }
            index += 1;
        }
        assert_eq!(rows, 271);
    }
}
