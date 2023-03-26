#![allow(missing_docs)]

use core::num::NonZeroU32;

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

#[inline]
pub fn bmp_get_header(bytes: &[u8]) -> Result<BmpHeader, ImagineError> {
  const SIZE_OF_FILE_HEADER: usize = 14;
  const SIZE_OF_U32: usize = 4;
  const MIN_FOR_INFO_HEADER_SIZE: usize = SIZE_OF_FILE_HEADER + SIZE_OF_U32;
  let rest = if bytes.len() >= MIN_FOR_INFO_HEADER_SIZE {
    &bytes[SIZE_OF_FILE_HEADER..]
  } else {
    return Err(ImagineError::ParseError);
  };
  let mut header = BmpHeader::default();
  let info_header_size = u32_le(&rest[0..4]);
  Ok(match info_header_size {
    40 => {
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
        let high = 4 * num_palette_entries;
        header.palette_span = Some((low, high));
      }
      header.image_span = {
        let start = u32_le(&bytes[10..14]).try_into().unwrap();
        let end = match onz_u32_le(&a[20..24]) {
          None => {
            start + {
              let width_u: usize = header.width.try_into().unwrap();
              let height_u: usize = header.height.try_into().unwrap();
              let bits_per_line = width_u.saturating_mul(header.bits_per_pixel.try_into().unwrap());
              let bytes_per_line_no_padding =
                (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
              let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
                + usize::from((bytes_per_line_no_padding % 4) != 0))
              .saturating_mul(4);
              height_u.saturating_mul(bytes_per_line_padded)
            }
          }
          Some(nz) => nz.get().try_into().unwrap(),
        };
        (start, end)
      };
      header
    }
    124 => {
      let (a, rest) = try_pull_byte_array::<40>(rest)?;
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
        let high = 4 * num_palette_entries;
        header.palette_span = Some((low, high));
      }
      header.image_span = {
        let start = u32_le(&bytes[10..14]).try_into().unwrap();
        let end = match onz_u32_le(&a[20..24]) {
          None => {
            start + {
              let width_u: usize = header.width.try_into().unwrap();
              let height_u: usize = header.height.try_into().unwrap();
              let bits_per_line = width_u.saturating_mul(header.bits_per_pixel.try_into().unwrap());
              let bytes_per_line_no_padding =
                (bits_per_line / 8) + usize::from((bits_per_line % 8) != 0);
              let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
                + usize::from((bytes_per_line_no_padding % 4) != 0))
              .saturating_mul(4);
              height_u.saturating_mul(bytes_per_line_padded)
            }
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
      header
    }
    _ => return Err(ImagineError::ParseError),
  })
}
