#![forbid(unsafe_code)]

//! Module for Windows Bitmap files (BMP).
//!
//! ## Parsing The Format
//!
//! Note: All multi-byte values in BMP are always little-endian encoded.
//!
//! * A bitmap file always starts with a "file header". This is always 14 bytes.
//!   * A tag for the kind of bitmap you're expected to find
//!   * A total size of the file, to check if a file was unexpectedly truncated
//!   * The position of the bitmap data within the file. However, without
//!     knowing more you probably can't use this position directly.
//! * Next is an "info header". There's many versions of this header. The first
//!   4 bytes are always the size of the full info header, and each version is a
//!   different size, so this lets you figure out what version is being used for
//!   this file.
//! * Next there **might** be bitmask data. If the header is an InfoHeader and
//!   the compression is [BmpCompression::Bitfields] or
//!   [BmpCompression::AlphaBitfields] then there will be 3 or 4 `u32` values
//!   that specify the bit regions for the R, G, B, and possibly A data. These
//!   compression formats should only appear with 16 or 32 bpp images.
//! * Next there **might** be a color table. This is mandatory if the bit depth
//!   is 8 (or less) bits per pixel, and it's optional otherwise. The default
//!   number of entries is `2**bits_per_pixel`, though most header formats can
//!   optionally specify a smaller color table. Each entry in the color table is
//!   generally a `[u8;4]` value (`[b, g, r, 0]`), except if `BmpInfoHeaderCore`
//!   is used, in which case each entry is a `[u8;3]` value (`[b,g,r]`). There's
//!   apparently a 2 byte per entry form (indexing into the current system
//!   palette) that's supposed to only be used while the image is in memory on
//!   older systems, so you shouldn't find it in files. If the image supports
//!   alpha, then the color table might contain non-zero alpha values. If all
//!   alpha values in the color table are 0 you should assume that the color
//!   table colors are intended to be fully opaque.
//! * Next there **might** be a gap in the data. This allows the pixel data to
//!   be re-aligned to 4 (if necessary), though this assumes that the file
//!   itself was loaded into memory at an alignment of at least 4. The offset of
//!   the pixel array was given in the file header, use that to skip past the
//!   gap (if any).
//! * Next there is the pixel array. This data format depends on the compression
//!   style used, as defined in the bitmap header. Each row of the bitmap is
//!   supposedly padded to 4 bytes.
//! * Next there **might** be another gap region.
//! * Finally there is the ICC color profile data, if any. The format of this
//!   data changes depending on what was specified in the bitmap header.
//!
//! When the bits per pixel is less than 8 the pixels will be packed within a
//! byte. In this case, the leftmost pixel is the highest bits of the byte.
//! * 1, 2, 4, and 8 bits per pixel are indexed color.
//! * 16 and 32 bits per pixel is direct color, with the bitmasks defining the
//!   location of each channel within a (little-endian) `u16` or `u32`.
//! * 24 bits per pixel is direct color and the channel order is always implied
//!   to be `[b,g,r]` within `[u8; 3]`.

use crate::AsciiArray;
use core::num::{NonZeroU16, NonZeroU32};

mod headers;
pub use headers::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BmpError {
  ThisIsProbablyNotABmpFile,
  InsufficientBytes,
  IncorrectSizeForThisInfoHeaderVersion,
  UnknownCompression,
  UnknownHeaderLength,
  /// The BMP file might be valid, but either way this library doesn't currently
  /// know how to parse it.
  ParserIncomplete,
}
