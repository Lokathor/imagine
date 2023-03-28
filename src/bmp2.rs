#![allow(missing_docs)]

use core::num::{NonZeroU32, NonZeroU8};

use bitfrob::{U8BitIterHigh, U8BitIterLow};
use bytemuck::{cast_slice, try_cast_slice};
use pixel_formats::{r32g32b32_Sfloat, r32g32b32a32_Sfloat, r8g8b8_Srgb, r8g8b8a8_Srgb};

use crate::{sRGBIntent, util::*, ImagineError};

/// Header data from a bitmap file.
///
/// A full BMP header is split into the "file header" and "info header", and
/// there's at least 7(?) different versions of the info header. Many fields
/// exist that only have a single possible value, or that have mostly useless
/// information (eg: intended physical dimensions of the image).
///
/// This structure collects all the important information and gives it a
/// consistent presentation.
///
/// **Alpha:** If the image uses alpha (either "alpha bitfields" compression, or
/// 32bpp with no compression), the *expectation* is that the color and alpha
/// channels are pre-multiplied. Not all image editors necessarily do this, so
/// some files might have straight alpha anyway.
#[derive(Debug, Clone, Copy, Default)]
pub struct BmpHeader {
  /// Image width in pixels.
  pub width: u32,

  /// Image height in pixels.
  pub height: u32,

  /// Bits per pixel, from 1 to 32.
  ///
  /// * 1, 2, 4, or 8: paletted color (possibly with RLE)
  /// * 16 or 32: bitmask or alpha-bitmask color
  /// * 24: `[b,g,r]` color
  pub bits_per_pixel: usize,

  /// Byte span where the palette is, if any.
  pub palette_span: Option<(usize, usize)>,

  /// Byte span where the image data is.
  pub image_span: (usize, usize),

  /// The file's compression scheme, if any.
  pub compression: Option<BmpCompression>,

  /// The sRGB intent of the image, if any.
  pub srgb_intent: Option<sRGBIntent>,

  /// If the image's origin is the top left (otherwise it's the bottom left).
  pub origin_top_left: bool,
}
impl BmpHeader {
  /// If the image is using the alpha channel.
  #[inline]
  #[must_use]
  pub const fn uses_alpha(&self) -> bool {
    self.bits_per_pixel == 32
      || matches!(self.compression, Some(BmpCompression::AlphaBitfields { .. }))
  }

  /// This gets the palette entries from the bytes.
  ///
  /// * The entries are `[b, g, r, _]`, or `[b, g, r, a]` if the image uses
  ///   alpha.
  /// * When `srgb_intent` is set then the bytes are likely sRGB encoded.
  ///   Otherwise, they are likely linearly encoded.
  #[inline]
  #[must_use]
  pub fn get_palette<'b>(&self, bytes: &'b [u8]) -> Option<&'b [[u8; 4]]> {
    self.palette_span.and_then(|(low, high)| {
      if bytes.len() < high && low < high {
        try_cast_slice(&bytes[low..high]).ok()
      } else {
        None
      }
    })
  }

  /// Gets the bytes of the image data.
  #[inline]
  #[must_use]
  pub fn get_image_bytes<'b>(&self, bytes: &'b [u8]) -> Option<&'b [u8]> {
    let (low, high) = self.image_span;
    if bytes.len() < high && low < high {
      Some(&bytes[low..high])
    } else {
      None
    }
  }
}

/// Compression options for BMP files.
#[derive(Debug, Clone, Copy)]
pub enum BmpCompression {
  /// MSDN: [Bitmap Compression][1]
  ///
  /// [1]: https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-compression
  RunLengthEncoding,
  /// RGB bitfields
  #[allow(missing_docs)]
  Bitfields { red: u32, green: u32, blue: u32 },
  /// RGBA bitfields
  #[allow(missing_docs)]
  AlphaBitfields { red: u32, green: u32, blue: u32, alpha: u32 },
}

/// Checks if a BMP's initial 14 bytes are correct.
#[inline]
pub fn bmp_signature_is_correct(bytes: &[u8]) -> bool {
  match bytes {
    [b'B', b'M', s0, s1, s2, s3, _, _, _, _, p0, p1, p2, p3] => {
      let total_size = u32::from_be_bytes([*s0, *s1, *s2, *s3]);
      let px_offset = u32::from_be_bytes([*p0, *p1, *p2, *p3]);
      (total_size == bytes.len().try_into().unwrap()) && (px_offset < total_size)
    }
    _ => false,
  }
}

const SIZE_OF_FILE_HEADER: usize = 14;

#[inline]
pub fn bmp_get_header(bytes: &[u8]) -> Result<BmpHeader, ImagineError> {
  const SIZE_OF_U32: usize = 4;
  const MIN_FOR_INFO_HEADER_SIZE: usize = SIZE_OF_FILE_HEADER + SIZE_OF_U32;
  let rest = if bytes.len() >= MIN_FOR_INFO_HEADER_SIZE {
    &bytes[SIZE_OF_FILE_HEADER..]
  } else {
    return Err(ImagineError::ParseError);
  };
  let info_header_size = u32_le(&rest[0..4]);
  let i_start = u32_le(&bytes[10..14]).try_into().unwrap();
  match info_header_size {
    40 => try_header_v1(i_start, rest),
    124 => try_header_v5(i_start, rest),
    _ => Err(ImagineError::ParseError),
  }
}

fn try_header_v1(i_start: usize, rest: &[u8]) -> Result<BmpHeader, ImagineError> {
  let mut header = BmpHeader::default();
  let (a, rest) = try_pull_byte_array::<40>(rest)?;
  header.width = i32_le(&a[4..8]).unsigned_abs();
  header.height = i32_le(&a[8..12]).unsigned_abs();
  header.origin_top_left = i32_le(&a[8..12]).is_negative();
  header.bits_per_pixel = usize::from(u16_le(&a[14..16]));
  header.compression = match u32_le(&a[16..20]) {
    0 => None,
    // RLE4 and RLE8 are both one value in this lib.
    1 | 2 => Some(BmpCompression::RunLengthEncoding),
    3 => {
      let (masks, _) = try_pull_byte_array::<12>(rest)?;
      Some(BmpCompression::Bitfields {
        red: u32_le(&masks[0..4]),
        green: u32_le(&masks[4..8]),
        blue: u32_le(&masks[8..12]),
      })
    }
    6 => {
      let (masks, _) = try_pull_byte_array::<16>(rest)?;
      Some(BmpCompression::AlphaBitfields {
        red: u32_le(&masks[0..4]),
        green: u32_le(&masks[4..8]),
        blue: u32_le(&masks[8..12]),
        alpha: u32_le(&masks[12..16]),
      })
    }
    _ => return Err(ImagineError::ParseError),
  };
  let num_palette_entries: usize = onz_u32_le(&a[32..36])
    .map(NonZeroU32::get)
    .unwrap_or(if header.bits_per_pixel < 8 { 1 << header.bits_per_pixel } else { 0 })
    .try_into()
    .unwrap();
  if num_palette_entries > 0 {
    let low = SIZE_OF_FILE_HEADER
      + 40
      + match header.compression {
        Some(BmpCompression::Bitfields { .. }) => 12,
        Some(BmpCompression::AlphaBitfields { .. }) => 16,
        _ => 0,
      };
    let high = low + 4 * num_palette_entries;
    header.palette_span = Some((low, high));
  }
  header.image_span = {
    let start = i_start;
    let end = start
      + match onz_u32_le(&a[20..24]) {
        None => {
          let width_u: usize = header.width.try_into().unwrap();
          let height_u: usize = header.height.try_into().unwrap();
          let bits_per_line = width_u.saturating_mul(header.bits_per_pixel);
          let bytes_per_line_no_padding =
            (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
          let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
            + usize::from((bytes_per_line_no_padding % 4) != 0))
          .saturating_mul(4);
          height_u.saturating_mul(bytes_per_line_padded)
        }
        Some(nz) => nz.get().try_into().unwrap(),
      };
    (start, end)
  };
  Ok(header)
}

fn try_header_v5(i_start: usize, rest: &[u8]) -> Result<BmpHeader, ImagineError> {
  let mut header = BmpHeader::default();
  let (a, _rest) = try_pull_byte_array::<124>(rest)?;
  header.width = i32_le(&a[4..8]).unsigned_abs();
  header.height = i32_le(&a[8..12]).unsigned_abs();
  header.origin_top_left = i32_le(&a[8..12]).is_negative();
  header.bits_per_pixel = usize::from(u16_le(&a[14..16]));
  header.compression = match u32_le(&a[16..20]) {
    0 => None,
    // RLE4 and RLE8 are both one value in this lib.
    1 | 2 => Some(BmpCompression::RunLengthEncoding),
    3 => Some(BmpCompression::Bitfields {
      red: u32_le(&a[40..44]),
      green: u32_le(&a[44..48]),
      blue: u32_le(&a[48..52]),
    }),
    6 => Some(BmpCompression::AlphaBitfields {
      red: u32_le(&a[40..44]),
      green: u32_le(&a[44..48]),
      blue: u32_le(&a[48..52]),
      alpha: u32_le(&a[52..56]),
    }),
    _ => return Err(ImagineError::ParseError),
  };
  header.palette_span = {
    let num_palette_entries: usize = onz_u32_le(&a[32..36])
      .map(NonZeroU32::get)
      .unwrap_or(if header.bits_per_pixel < 8 { 1 << header.bits_per_pixel } else { 0 })
      .try_into()
      .unwrap();
    if num_palette_entries > 0 {
      let low = SIZE_OF_FILE_HEADER
        + 40
        + match header.compression {
          Some(BmpCompression::Bitfields { .. }) => 12,
          Some(BmpCompression::AlphaBitfields { .. }) => 16,
          _ => 0,
        };
      let high = low + 4 * num_palette_entries;
      Some((low, high))
    } else {
      None
    }
  };
  header.image_span = {
    let start = i_start;
    let end = start
      + match onz_u32_le(&a[20..24]) {
        None => {
          let width_u: usize = header.width.try_into().unwrap();
          let height_u: usize = header.height.try_into().unwrap();
          let bits_per_line = width_u.saturating_mul(header.bits_per_pixel);
          let bytes_per_line_no_padding =
            (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
          let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
            + usize::from((bytes_per_line_no_padding % 4) != 0))
          .saturating_mul(4);
          height_u.saturating_mul(bytes_per_line_padded)
        }
        Some(nz) => nz.get().try_into().unwrap(),
      };
    (start, end)
  };
  const LCS_GM_ABS_COLORIMETRIC: u32 = 0x00000008;
  const LCS_GM_BUSINESS: u32 = 0x00000001;
  const LCS_GM_GRAPHICS: u32 = 0x00000002;
  const LCS_GM_IMAGES: u32 = 0x00000004;
  header.srgb_intent = match u32_le(&a[108..112]) {
    LCS_GM_ABS_COLORIMETRIC => Some(sRGBIntent::AbsoluteColorimetric),
    LCS_GM_BUSINESS => Some(sRGBIntent::Saturation),
    LCS_GM_GRAPHICS => Some(sRGBIntent::RelativeColorimetric),
    LCS_GM_IMAGES => Some(sRGBIntent::Perceptual),
    _ => None,
  };
  Ok(header)
}

/// Iterate the palette indexes of the image bytes, based on the bit depth.
///
/// Only images with `bits_per_pixel` of 1, 2, 4, or 8 use the palette.
///
/// ## Panics
/// * The `bits_per_pixel` must be in the range `1..=8`.
#[inline]
pub fn bmp_iter_pal_indexes_no_compression(
  image_bytes: &[u8], bits_per_pixel: usize,
) -> impl Iterator<Item = usize> + '_ {
  assert!((1..=8).contains(&bits_per_pixel));
  let count = bits_per_pixel as u32;
  image_bytes
    .iter()
    .copied()
    .flat_map(move |bits| U8BitIterHigh::from_count_and_bits(count, bits))
    .map(usize::from)
}

#[derive(Debug, Clone, Copy)]
pub enum BmpRle8Op {
  Run { count: NonZeroU8, index: usize },
  Newline,
  EndOfBmp,
  Delta { right: u32, up: u32 },
  Raw2 { q: usize, w: usize },
  Raw1 { q: usize },
}

/// Iterate RLE encoded data, 8 bits per pixel
#[inline]
pub fn bmp_iter_rle8(image_bytes: &[u8]) -> impl Iterator<Item = BmpRle8Op> + '_ {
  // Now the MSDN docs get kinda terrible. They talk about "encoded" and
  // "absolute" mode, but whoever wrote that is bad at writing docs. What
  // we're doing is we'll pull off two bytes at a time from the pixel data.
  // Then we look at the first byte in a pair and see if it's zero or not.
  //
  // * If the first byte is **non-zero** it's the number of times that the second
  //   byte appears in the output. The second byte is an index into the palette,
  //   and you just put out that color and output it into the bitmap however many
  //   times.
  // * If the first byte is **zero**, it signals an "escape sequence" sort of
  //   situation. The second byte will give us the details:
  //   * 0: end of line
  //   * 1: end of bitmap
  //   * 2: "Delta", the *next* two bytes after this are unsigned offsets to the
  //     right and up of where the output should move to (remember that this mode
  //     always describes the BMP with a bottom-left origin).
  //   * 3+: "Absolute", The second byte gives a count of how many bytes follow
  //     that we'll output without repetition. The absolute output sequences
  //     always have a padding byte on the ending if the sequence count is odd, so
  //     we can keep pulling `[u8;2]` at a time from our data and it all works.
  let bytes: &[[u8; 2]] = cast_slice(image_bytes);
  let mut it = bytes.iter().copied();
  let mut raw_count = 0_u8;
  core::iter::from_fn(move || {
    if raw_count > 0 {
      let [q, w] = it.next()?;
      let out = if raw_count >= 2 {
        BmpRle8Op::Raw2 { q: usize::from(q), w: usize::from(w) }
      } else {
        BmpRle8Op::Raw1 { q: usize::from(q) }
      };
      raw_count = raw_count.saturating_sub(2);
      Some(out)
    } else {
      let [a, b] = it.next()?;
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle8Op::Run { count, index: usize::from(b) }),
        None => match b {
          0 => Some(BmpRle8Op::Newline),
          1 => Some(BmpRle8Op::EndOfBmp),
          2 => {
            let [right, up] = it.next()?;
            Some(BmpRle8Op::Delta { right: u32::from(right), up: u32::from(up) })
          }
          x => {
            let [q, w] = it.next()?;
            let out = BmpRle8Op::Raw2 { q: usize::from(q), w: usize::from(w) };
            raw_count = x.saturating_sub(2);
            Some(out)
          }
        },
      }
    }
  })
}

#[derive(Debug, Clone, Copy)]
pub enum BmpRle4Op {
  Run { count: NonZeroU8, index_h: usize, index_l: usize },
  Newline,
  EndOfBmp,
  Delta { right: u32, up: u32 },
  Raw4 { a: usize, b: usize, c: usize, d: usize },
  Raw3 { a: usize, b: usize, c: usize },
  Raw2 { a: usize, b: usize },
  Raw1 { a: usize },
}

/// Iterate RLE encoded data, 4 bits per pixel
#[inline]
pub fn bmp_iter_rle4(image_bytes: &[u8]) -> impl Iterator<Item = BmpRle4Op> + '_ {
  // RLE4 works *basically* how RLE8 does, except that every time we
  // process a byte as a color to output then it's actually two outputs
  // instead (upper bits then lower bits). The stuff about the escape
  // sequences and all that is still the same sort of thing.
  let bytes: &[[u8; 2]] = cast_slice(image_bytes);
  let mut it = bytes.iter().copied();
  let mut raw_count = 0_u8;
  core::iter::from_fn(move || {
    if raw_count > 0 {
      let [q, w] = it.next()?;
      let out = match raw_count {
        1 => BmpRle4Op::Raw1 { a: usize::from(q >> 4) },
        2 => BmpRle4Op::Raw2 { a: usize::from(q >> 4), b: usize::from(q & 0b1111) },
        3 => BmpRle4Op::Raw3 {
          a: usize::from(q >> 4),
          b: usize::from(q & 0b1111),
          c: usize::from(w >> 4),
        },
        _more => BmpRle4Op::Raw4 {
          a: usize::from(q >> 4),
          b: usize::from(q & 0b1111),
          c: usize::from(w >> 4),
          d: usize::from(w & 0b1111),
        },
      };
      raw_count = raw_count.saturating_sub(4);
      Some(out)
    } else {
      let [a, b] = it.next()?;
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle4Op::Run {
          count,
          index_h: usize::from(b >> 4),
          index_l: usize::from(b & 0b1111),
        }),
        None => match b {
          0 => Some(BmpRle4Op::Newline),
          1 => Some(BmpRle4Op::EndOfBmp),
          2 => {
            let [right, up] = it.next()?;
            Some(BmpRle4Op::Delta { right: u32::from(right), up: u32::from(up) })
          }
          x => {
            let [q, w] = it.next()?;
            let out = match raw_count {
              3 => BmpRle4Op::Raw3 {
                a: usize::from(q >> 4),
                b: usize::from(q & 0b1111),
                c: usize::from(w >> 4),
              },
              _more => BmpRle4Op::Raw4 {
                a: usize::from(q >> 4),
                b: usize::from(q & 0b1111),
                c: usize::from(w >> 4),
                d: usize::from(w & 0b1111),
              },
            };
            raw_count = x.saturating_sub(4);
            Some(out)
          }
        },
      }
    }
  })
}

/// Iterates 24bpp BGR data in the image bytes.
///
/// The encoding of the `u8` values depends on if the image is sRGB or not. If
/// the image is not sRGB then it's most likely linear values in each channel.
///
/// ## Panics
/// * The `image_bytes` must have a length that's a multiple of 3.
#[inline]
pub fn bmp_iter_bgr24(image_bytes: &[u8]) -> impl Iterator<Item = [u8; 3]> + '_ {
  let bgr = cast_slice(image_bytes);
  bgr.iter().copied()
}

/// Iterates 16-bits-per-pixel values using the RGB bitmasks given.
#[inline]
pub fn bmp_iter_bitmask16_rgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let u16_le_bytes: &[[u8; 2]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  u16_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from(u16::from_le_bytes(bytes));
    let r_bits = (u & r_mask) >> r_shift;
    let g_bits = (u & g_mask) >> g_shift;
    let b_bits = (u & b_mask) >> b_shift;
    let r = (r_bits as f32) / r_max_f32;
    let g = (g_bits as f32) / g_max_f32;
    let b = (b_bits as f32) / b_max_f32;
    r32g32b32_Sfloat { r, g, b }
  })
}

/// Iterates 16-bits-per-pixel values using the RGBA bitmasks given.
#[inline]
pub fn bmp_iter_bitmask16_rgba(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
  let u16_le_bytes: &[[u8; 2]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let a_shift = a_mask.trailing_zeros();
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let a_max = a_mask >> a_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let a_max_f32 = a_max as f32;
  u16_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from(u16::from_le_bytes(bytes));
    let r_bits = (u & r_mask) >> r_shift;
    let g_bits = (u & g_mask) >> g_shift;
    let b_bits = (u & b_mask) >> b_shift;
    let a_bits = (u & a_mask) >> a_shift;
    let r = (r_bits as f32) / r_max_f32;
    let g = (g_bits as f32) / g_max_f32;
    let b = (b_bits as f32) / b_max_f32;
    let a = (a_bits as f32) / a_max_f32;
    r32g32b32a32_Sfloat { r, g, b, a }
  })
}

/// Iterates 32-bits-per-pixel linear values using the RGB bitmasks given.
#[inline]
pub fn bmp_iter_bitmask32_linear_rgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let u32_le_bytes: &[[u8; 4]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  u32_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from_le_bytes(bytes);
    let r_bits = (u & r_mask) >> r_shift;
    let g_bits = (u & g_mask) >> g_shift;
    let b_bits = (u & b_mask) >> b_shift;
    let r = (r_bits as f32) / r_max_f32;
    let g = (g_bits as f32) / g_max_f32;
    let b = (b_bits as f32) / b_max_f32;
    r32g32b32_Sfloat { r, g, b }
  })
}

/// Iterates 32-bits-per-pixel linear values using the RGBA bitmasks given.
#[inline]
pub fn bmp_iter_bitmask32_linear_rgba(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
  let u32_le_bytes: &[[u8; 4]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let a_shift = a_mask.trailing_zeros();
  let r_max_f32 = (r_mask >> r_shift) as f32;
  let g_max_f32 = (g_mask >> g_shift) as f32;
  let b_max_f32 = (b_mask >> b_shift) as f32;
  let a_max_f32 = (a_mask >> a_shift) as f32;
  u32_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from_le_bytes(bytes);
    let r_bits = (u & r_mask) >> r_shift;
    let g_bits = (u & g_mask) >> g_shift;
    let b_bits = (u & b_mask) >> b_shift;
    let a_bits = (u & a_mask) >> a_shift;
    let r = (r_bits as f32) / r_max_f32;
    let g = (g_bits as f32) / g_max_f32;
    let b = (b_bits as f32) / b_max_f32;
    let a = (a_bits as f32) / a_max_f32;
    r32g32b32a32_Sfloat { r, g, b, a }
  })
}

/// Iterates 32-bits-per-pixel sRGB using the RGB bitmasks given.
///
/// It's assumed that each mask is 8 bits big, results will be weird if this is
/// not the case.
#[inline]
pub fn bmp_iter_bitmask32_srgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32,
) -> impl Iterator<Item = r8g8b8_Srgb> + '_ {
  let u32_le_bytes: &[[u8; 4]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  u32_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from_le_bytes(bytes);
    let r = ((u & r_mask) >> r_shift) as u8;
    let g = ((u & g_mask) >> g_shift) as u8;
    let b = ((u & b_mask) >> b_shift) as u8;
    r8g8b8_Srgb { r, g, b }
  })
}

/// Iterates 32-bits-per-pixel sRGBA using the RGBA bitmasks given.
///
/// It's assumed that each mask is 8 bits big, results will be weird if this is
/// not the case.
#[inline]
pub fn bmp_iter_bitmask32_srgba(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32,
) -> impl Iterator<Item = r8g8b8a8_Srgb> + '_ {
  let u32_le_bytes: &[[u8; 4]] = cast_slice(image_bytes);
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let a_shift = a_mask.trailing_zeros();
  u32_le_bytes.iter().copied().map(move |bytes| {
    let u = u32::from_le_bytes(bytes);
    let r = ((u & r_mask) >> r_shift) as u8;
    let g = ((u & g_mask) >> g_shift) as u8;
    let b = ((u & b_mask) >> b_shift) as u8;
    let a = ((u & a_mask) >> a_shift) as u8;
    r8g8b8a8_Srgb { r, g, b, a }
  })
}
