#![allow(missing_docs)]

use core::num::{NonZeroU32, NonZeroU8};

use bitfrob::{U8BitIterHigh, U8BitIterLow};
use bytemuck::{cast_slice, try_cast_slice};
use pixel_formats::*;

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
  pub fn get_palette<'b>(&self, bytes: &'b [u8]) -> Result<&'b [[u8; 4]], ImagineError> {
    self
      .palette_span
      .and_then(|(low, high)| {
        if low < high && high <= bytes.len() {
          try_cast_slice(&bytes[low..high]).ok()
        } else {
          None
        }
      })
      .ok_or(ImagineError::Value)
  }

  /// Gets the bytes of the image data.
  #[inline]
  pub fn get_image_bytes<'b>(&self, bytes: &'b [u8]) -> Result<&'b [u8], ImagineError> {
    let (low, high) = self.image_span;
    if low < high && high <= bytes.len() {
      Ok(&bytes[low..high])
    } else {
      Err(ImagineError::Value)
    }
  }

  /// Runs the `(x,y,index)` op for all pixels.
  ///
  /// The run-length encoding compression can cause pixels to be handled out of
  /// order, and so the operation is always given the `(x,y)` affected.
  ///
  /// ## Failure
  /// * The bit depth and compression combination must be one of:
  ///   * 1, 2, 4, or 8 with no compression
  ///   * 4 or 8 with `RunLengthEncoding` compression
  #[inline]
  pub fn for_each_pal_index<F: FnMut(u32, u32, u8)>(
    &self, bytes: &[u8], mut op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      width,
      height,
      bits_per_pixel,
      compression,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
      srgb_intent: _,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match bits_per_pixel {
      1 | 2 | 4 | 8 if compression.is_none() => {
        let index_iter = bmp_iter_pal_indexes_no_compression(
          image_bytes,
          *bits_per_pixel,
          (*width).try_into().unwrap(),
        );
        (0..*height)
          .flat_map(|y| (0..*width).map(move |x| (x, y)))
          .zip(index_iter)
          .for_each(|((x, y), p)| op(x, y, p))
      }
      4 if *compression == Some(BmpCompression::RunLengthEncoding) => {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        for rle_op in bmp_iter_rle4(image_bytes) {
          match rle_op {
            BmpRle4Op::EndOfBmp => return Ok(()),
            BmpRle4Op::Newline => {
              x = 0;
              y = y.wrapping_add(1);
            }
            BmpRle4Op::Run { count, index_h, index_l } => {
              let mut it = [index_h, index_l].into_iter().cycle();
              for _ in 0..count.get() {
                op(x, y, it.next().unwrap());
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Delta { right, up } => {
              x = x.wrapping_add(right);
              y = y.wrapping_add(up);
            }
            BmpRle4Op::Raw4 { a, b, c, d } => {
              for val in [a, b, c, d] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw3 { a, b, c } => {
              for val in [a, b, c] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw2 { a, b } => {
              for val in [a, b] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw1 { a } => {
              op(x, y, a);
              x = x.wrapping_add(1);
            }
          }
        }
      }
      8 if *compression == Some(BmpCompression::RunLengthEncoding) => {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        for rle_op in bmp_iter_rle8(image_bytes) {
          match rle_op {
            BmpRle8Op::EndOfBmp => return Ok(()),
            BmpRle8Op::Newline => {
              x = 0;
              y = y.wrapping_add(1);
            }
            BmpRle8Op::Run { count, index } => {
              for _ in 0..count.get() {
                op(x, y, index);
                x = x.wrapping_add(1);
              }
            }
            BmpRle8Op::Delta { right, up } => {
              x = x.wrapping_add(right);
              y = y.wrapping_add(up);
            }
            BmpRle8Op::Raw2 { q, w } => {
              op(x, y, q);
              x = x.wrapping_add(1);
              op(x, y, w);
              x = x.wrapping_add(1);
            }
            BmpRle8Op::Raw1 { q } => {
              op(x, y, q);
              x = x.wrapping_add(1);
            }
          }
        }
      }
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }

  /// Runs the op for all pixels
  ///
  /// Pixels proceed left to right across each scan line. Depending on the
  /// `origin_top_left` value in the header the scanlines proceed top down, or
  /// bottom up.
  ///
  /// ## Failure
  /// * The bit depth and compression combination must be one of:
  ///   * 24 with no compression
  ///   * 16 or 32, and `Bitfields` compression
  #[inline]
  pub fn for_each_rgb<F: FnMut(r32g32b32_Sfloat)>(
    &self, bytes: &[u8], op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      width,
      height: _,
      bits_per_pixel,
      compression,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
      srgb_intent,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match (bits_per_pixel, compression) {
      (24, None) => {
        let bgr_it = bmp_iter_bgr24(image_bytes, (*width).try_into().unwrap());
        if srgb_intent.is_some() {
          bgr_it.map(|[b, g, r]| r32g32b32_Sfloat::from(r8g8b8_Srgb { r, g, b })).for_each(op)
        } else {
          bgr_it.map(|[b, g, r]| r32g32b32_Sfloat::from(r8g8b8_Unorm { r, g, b })).for_each(op)
        }
      }
      (16, Some(BmpCompression::Bitfields { r_mask, g_mask, b_mask })) => {
        bmp_iter_bitmask16_rgb(image_bytes, *r_mask, *g_mask, *b_mask, (*width).try_into().unwrap())
          .for_each(op)
      }
      (32, Some(BmpCompression::Bitfields { r_mask, g_mask, b_mask })) => {
        if srgb_intent.is_some() {
          bmp_iter_bitmask32_srgb(
            image_bytes,
            *r_mask,
            *g_mask,
            *b_mask,
            (*width).try_into().unwrap(),
          )
          .map(r32g32b32_Sfloat::from)
          .for_each(op)
        } else {
          bmp_iter_bitmask32_linear_rgb(
            image_bytes,
            *r_mask,
            *g_mask,
            *b_mask,
            (*width).try_into().unwrap(),
          )
          .map(r32g32b32_Sfloat::from)
          .for_each(op)
        }
      }
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }

  /// Runs the op for all pixels
  ///
  /// Pixels proceed left to right across each scan line. Depending on the
  /// `origin_top_left` value in the header the scanlines proceed top down, or
  /// bottom up.
  ///
  /// ## Failure
  /// * The bit depth must be 16 or 32, and the compression must be
  ///   `AlphaBitfields`
  #[inline]
  pub fn for_each_rgba<F: FnMut(r32g32b32a32_Sfloat)>(
    &self, bytes: &[u8], op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      bits_per_pixel,
      compression,
      srgb_intent,
      width,
      height: _,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match (bits_per_pixel, compression) {
      (16, Some(BmpCompression::AlphaBitfields { r_mask, g_mask, b_mask, a_mask })) => {
        bmp_iter_bitmask16_rgba(
          image_bytes,
          *r_mask,
          *g_mask,
          *b_mask,
          *a_mask,
          (*width).try_into().unwrap(),
        )
        .for_each(op)
      }
      (32, Some(BmpCompression::AlphaBitfields { r_mask, g_mask, b_mask, a_mask })) => {
        if srgb_intent.is_some() {
          bmp_iter_bitmask32_srgba(
            image_bytes,
            *r_mask,
            *g_mask,
            *b_mask,
            *a_mask,
            (*width).try_into().unwrap(),
          )
          .map(r32g32b32a32_Sfloat::from)
          .for_each(op)
        } else {
          bmp_iter_bitmask32_linear_rgba(
            image_bytes,
            *r_mask,
            *g_mask,
            *b_mask,
            *a_mask,
            (*width).try_into().unwrap(),
          )
          .map(r32g32b32a32_Sfloat::from)
          .for_each(op)
        }
      }
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }
}

/// Compression options for BMP files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmpCompression {
  /// MSDN: [Bitmap Compression][1]
  ///
  /// [1]: https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-compression
  RunLengthEncoding,
  /// RGB bitfields
  #[allow(missing_docs)]
  Bitfields { r_mask: u32, g_mask: u32, b_mask: u32 },
  /// RGBA bitfields
  #[allow(missing_docs)]
  AlphaBitfields { r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32 },
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
    return Err(ImagineError::Parse);
  };
  let info_header_size = u32_le(&rest[0..4]);
  let i_start = u32_le(&bytes[10..14]).try_into().unwrap();
  match info_header_size {
    40 => try_header_v1(i_start, rest),
    124 => try_header_v5(i_start, rest),
    _ => Err(ImagineError::Parse),
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
        r_mask: u32_le(&masks[0..4]),
        g_mask: u32_le(&masks[4..8]),
        b_mask: u32_le(&masks[8..12]),
      })
    }
    6 => {
      let (masks, _) = try_pull_byte_array::<16>(rest)?;
      Some(BmpCompression::AlphaBitfields {
        r_mask: u32_le(&masks[0..4]),
        g_mask: u32_le(&masks[4..8]),
        b_mask: u32_le(&masks[8..12]),
        a_mask: u32_le(&masks[12..16]),
      })
    }
    _ => return Err(ImagineError::Parse),
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
      r_mask: u32_le(&a[40..44]),
      g_mask: u32_le(&a[44..48]),
      b_mask: u32_le(&a[48..52]),
    }),
    6 => Some(BmpCompression::AlphaBitfields {
      r_mask: u32_le(&a[40..44]),
      g_mask: u32_le(&a[44..48]),
      b_mask: u32_le(&a[48..52]),
      a_mask: u32_le(&a[52..56]),
    }),
    _ => return Err(ImagineError::Parse),
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
  image_bytes: &[u8], bits_per_pixel: usize, width: usize,
) -> impl Iterator<Item = u8> + '_ {
  assert!((1..=8).contains(&bits_per_pixel));
  let bits_per_line: usize = bits_per_pixel.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  let count = bits_per_pixel as u32;
  image_bytes.chunks(padded_bytes_per_line).flat_map(move |line| {
    line
      .iter()
      .copied()
      .flat_map(move |bits| U8BitIterHigh::from_count_and_bits(count, bits))
      .take(width)
  })
}

#[derive(Debug, Clone, Copy)]
pub enum BmpRle8Op {
  Run {
    count: NonZeroU8,
    index: u8,
  },
  /// This is the *only* time that the current scanline changes.
  Newline,
  EndOfBmp,
  Delta {
    right: u32,
    up: u32,
  },
  Raw2 {
    q: u8,
    w: u8,
  },
  Raw1 {
    q: u8,
  },
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
      let out = if raw_count >= 2 { BmpRle8Op::Raw2 { q, w } } else { BmpRle8Op::Raw1 { q } };
      raw_count = raw_count.saturating_sub(2);
      Some(out)
    } else {
      let [a, b] = it.next()?;
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle8Op::Run { count, index: b }),
        None => match b {
          0 => Some(BmpRle8Op::Newline),
          1 => Some(BmpRle8Op::EndOfBmp),
          2 => {
            let [right, up] = it.next()?;
            Some(BmpRle8Op::Delta { right: u32::from(right), up: u32::from(up) })
          }
          x => {
            let [q, w] = it.next()?;
            let out = BmpRle8Op::Raw2 { q, w };
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
  Run { count: NonZeroU8, index_h: u8, index_l: u8 },
  Newline,
  EndOfBmp,
  Delta { right: u32, up: u32 },
  Raw4 { a: u8, b: u8, c: u8, d: u8 },
  Raw3 { a: u8, b: u8, c: u8 },
  Raw2 { a: u8, b: u8 },
  Raw1 { a: u8 },
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
        1 => BmpRle4Op::Raw1 { a: q >> 4 },
        2 => BmpRle4Op::Raw2 { a: q >> 4, b: q & 0b1111 },
        3 => BmpRle4Op::Raw3 { a: q >> 4, b: q & 0b1111, c: w >> 4 },
        _more => BmpRle4Op::Raw4 { a: q >> 4, b: q & 0b1111, c: w >> 4, d: w & 0b1111 },
      };
      raw_count = raw_count.saturating_sub(4);
      Some(out)
    } else {
      let [a, b] = it.next()?;
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle4Op::Run { count, index_h: b >> 4, index_l: b & 0b1111 }),
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
              3 => BmpRle4Op::Raw3 { a: q >> 4, b: q & 0b1111, c: w >> 4 },
              _more => BmpRle4Op::Raw4 { a: q >> 4, b: q & 0b1111, c: w >> 4, d: w & 0b1111 },
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
pub fn bmp_iter_bgr24(image_bytes: &[u8], width: usize) -> impl Iterator<Item = [u8; 3]> + '_ {
  let bits_per_line: usize = 24_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes.chunks(padded_bytes_per_line).flat_map(move |line| {
    line.chunks(3).map(|c| <[u8; 3]>::try_from(c).unwrap_or_default()).take(width)
  })
}

/// Iterates 16-bits-per-pixel values using the RGB bitmasks given.
#[inline]
pub fn bmp_iter_bitmask16_rgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, width: usize,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
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
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32, width: usize,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
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
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
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
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, width: usize,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
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
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32, width: usize,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let a_shift = a_mask.trailing_zeros();
  let r_max_f32 = (r_mask >> r_shift) as f32;
  let g_max_f32 = (g_mask >> g_shift) as f32;
  let b_max_f32 = (b_mask >> b_shift) as f32;
  let a_max_f32 = (a_mask >> a_shift) as f32;
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
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
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, width: usize,
) -> impl Iterator<Item = r8g8b8_Srgb> + '_ {
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
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
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32, width: usize,
) -> impl Iterator<Item = r8g8b8a8_Srgb> + '_ {
  let r_shift = r_mask.trailing_zeros();
  let g_shift = g_mask.trailing_zeros();
  let b_shift = b_mask.trailing_zeros();
  let a_shift = a_mask.trailing_zeros();
  let bits_per_line: usize = 16_usize.saturating_mul(width);
  let no_padding_bytes_per_line: usize =
    (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
  let padded_bytes_per_line: usize =
    ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
  image_bytes
    .chunks(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks(2)
        .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap_or_default())))
        .take(width)
    })
    .map(move |u| {
      let r = ((u & r_mask) >> r_shift) as u8;
      let g = ((u & g_mask) >> g_shift) as u8;
      let b = ((u & b_mask) >> b_shift) as u8;
      let a = ((u & a_mask) >> a_shift) as u8;
      r8g8b8a8_Srgb { r, g, b, a }
    })
}

/// Automatically allocate and fill in a [Bitmap](crate::image::Bitmap).
///
/// * Paletted images will automatically get the color from the palette (illegal
///   palette index values will be black).
/// * RBG images with or without compression will be processed.
/// * RGBA images will fail.
///
/// The output is automatically flipped as necessary so that the output will be
/// oriented with the origin in the top left.
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn bmp_try_bitmap_rgb<P>(
  bytes: &[u8], origin_top_left: bool,
) -> Result<crate::image::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32_Sfloat>,
{
  use alloc::vec::Vec;

  let header = bmp_get_header(bytes)?;
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut bitmap = {
    let mut pixels = Vec::new();
    pixels.try_reserve(target_pixel_count)?;
    crate::image::Bitmap { width: header.width, height: header.height, pixels }
  };
  if header.bits_per_pixel <= 8 {
    // If we make a 256 element palette then indexing into the palette with a u8
    // will tend to optimize away the bounds check, and it usually goes much
    // faster than using `.get(i).unwrap_or_default()` or similar.
    let mut palette: [P; 256] = [r32g32b32_Sfloat::BLACK.into(); 256];
    let pal_bytes = header.get_palette(bytes)?;
    if header.srgb_intent.is_some() {
      for ([b, g, r, _], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
        *p = P::from(r32g32b32_Sfloat::from(r8g8b8_Srgb { r, g, b }));
      }
    } else {
      for ([b, g, r, _], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
        *p = P::from(r32g32b32_Sfloat::from(r8g8b8_Unorm { r, g, b }));
      }
    }
    let black: P = P::from(r32g32b32_Sfloat::BLACK);
    bitmap.pixels.resize(target_pixel_count, black);
    header.for_each_pal_index(bytes, |x, y, i| {
      if let Some(p_mut) = bitmap.get_mut(x, y) {
        *p_mut = palette[usize::from(i)];
      }
    })?;
  } else {
    header.for_each_rgb(bytes, |p| bitmap.pixels.push(p.into()))?;
  }
  let black: P = P::from(r32g32b32_Sfloat::BLACK);
  bitmap.pixels.resize(target_pixel_count, black);
  if header.origin_top_left != origin_top_left {
    bitmap.vertical_flip();
  }
  Ok(bitmap)
}

/// Automatically allocate and fill in a [Bitmap](crate::image::Bitmap).
///
/// * Paletted images will automatically get the color from the palette (illegal
///   palette index values will be transparent black).
/// * RBG images with or without compression will be processed, an alpha value
///   of 1.0 is automatically added.
/// * RGBA images with or without compression will be processed.
///
/// The output is automatically flipped as necessary so that the output will be
/// oriented with the origin in the top left.
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn bmp_try_bitmap_rgba<P>(
  bytes: &[u8], origin_top_left: bool,
) -> Result<crate::image::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32a32_Sfloat>,
{
  use alloc::vec::Vec;

  let header = bmp_get_header(bytes)?;
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut bitmap = {
    let mut pixels = Vec::new();
    pixels.try_reserve(target_pixel_count)?;
    crate::image::Bitmap { width: header.width, height: header.height, pixels }
  };
  if header.bits_per_pixel <= 8 {
    // If we make a 256 element palette then indexing into the palette with a u8
    // will tend to optimize away the bounds check, and it usually goes much
    // faster than using `.get(i).unwrap_or_default()` or similar.
    let mut palette: [P; 256] = [r32g32b32a32_Sfloat::TRANSPARENT_BLACK.into(); 256];
    let pal_bytes = header.get_palette(bytes)?;
    // When *none* of the palette entries have non-zero alpha we assume that the
    // entire palette is RGBX rather than RGBA, so we interpret all values as
    // full-opacity.
    let palette_has_no_alpha_values = pal_bytes.iter().map(|[_, _, _, a]| *a).all(|a| a == 0);
    if palette_has_no_alpha_values {
      if header.srgb_intent.is_some() {
        for ([b, g, r, _], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
          *p = P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a: u8::MAX }));
        }
      } else {
        for ([b, g, r, _], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
          *p = P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Unorm { r, g, b, a: u8::MAX }));
        }
      }
    } else if header.srgb_intent.is_some() {
      for ([b, g, r, a], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
        *p = P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a }));
      }
    } else {
      for ([b, g, r, a], p) in pal_bytes.iter().copied().zip(palette.iter_mut()) {
        *p = P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Unorm { r, g, b, a }));
      }
    }
    // RLE encoding doesn't touch all pixels. Untouched pixels are assumed to be
    // opaque black.
    let black: P = P::from(r32g32b32a32_Sfloat::OPAQUE_BLACK);
    bitmap.pixels.resize(target_pixel_count, black);
    header.for_each_pal_index(bytes, |x, y, i| {
      if let Some(p_mut) = bitmap.get_mut(x, y) {
        *p_mut = palette[usize::from(i)];
      }
    })?;
  } else {
    header.for_each_rgb(bytes, |p| bitmap.pixels.push(r32g32b32a32_Sfloat::from(p).into()))?;
  }
  let black: P = P::from(r32g32b32a32_Sfloat::TRANSPARENT_BLACK);
  bitmap.pixels.resize(target_pixel_count, black);
  if header.origin_top_left != origin_top_left {
    bitmap.vertical_flip();
  }
  Ok(bitmap)
}
