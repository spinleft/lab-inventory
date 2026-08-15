//! Driving a Brother QL-series label printer over raw TCP.
//!
//! The client sends a finished 1-bit bitmap; everything here turns that bitmap
//! into the printer's raster command language and puts it on the wire. Nothing
//! in this module touches HTTP or the database, so the protocol can be tested
//! without either.
//!
//! Layout mirrors the protocol's own layers:
//! - [`media`] — the physical label sizes and where the printable area sits
//! - [`raster`] — command encoding and PackBits compression
//! - [`status`] — the status block the printer reports back
//! - [`transport`] — connecting, address policy, and timeouts
pub mod media;
pub mod raster;
pub mod status;
pub mod transport;

/// Print head width in dots. Constant across the QL-800 family at 300 dpi.
pub const PINS: u16 = 720;

/// Bytes in one full-width raster row ([`PINS`] / 8).
pub const BYTES_PER_ROW: usize = 90;

/// Zero bytes that clear a half-finished job out of the printer's buffer.
///
/// The QL-820NWB wants 400 rather than the 200 older models accept.
pub const INVALIDATE_BYTES: usize = 400;

/// Shortest page the printer will accept, in raster lines.
pub const MIN_RASTER_LINES: u32 = 150;

/// Longest page the printer will accept, in raster lines.
pub const MAX_RASTER_LINES: u32 = 11811;

/// The raw printing port essentially every network printer listens on.
pub const DEFAULT_PRINTER_PORT: u16 = 9100;

/// Printers do not serve from privileged ports, so registrations that name one
/// are refused rather than used to probe the printer's host.
pub const MIN_PRINTER_PORT: u16 = 1024;
