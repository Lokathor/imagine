//! Module for BMP files.

use crate::{util::*, ImagineError};
use bitfrob::U8BitIterHigh;
use core::{mem::size_of, num::NonZeroU8};
use pack1::U32LE;
use pixel_formats::*;

pub mod iters;
pub mod nice_header;
pub mod raw_headers;
pub mod rle;

use self::{iters::*, nice_header::*, raw_headers::*, rle::*};

/// Checks if a BMP's initial 14 bytes are correct.
#[inline]
pub fn bmp_signature_is_correct(bytes: &[u8]) -> bool {
  if let Ok((file_header, _)) = try_pull_pod::<BitmapFileHeader>(bytes) {
    file_header.ty == "BM"
      && file_header.file_size.get().try_into().unwrap_or(usize::MAX) == bytes.len()
      && file_header.bitmap_offset.get().try_into().unwrap_or(usize::MAX) <= bytes.len()
  } else {
    false
  }
}

/// Computes the number of bytes per line when padding is applied.
///
/// BMP scanlines within the image data are padded to a multiple of 4 bytes
/// (except when the image is RLE encoded).
#[inline]
pub fn padded_bytes_per_line(width: u32, bits_per_pixel: u16) -> Result<usize, ImagineError> {
  let width: usize = width.try_into()?;
  let bits_per_pixel = usize::from(bits_per_pixel);
  let bits_per_line = bits_per_pixel.checked_mul(width).ok_or(ImagineError::Value)?;
  let bytes_per_line = bits_per_line / 8 + usize::from(bits_per_line % 8 != 0);
  let dwords_per_line = bytes_per_line / 4 + usize::from(bytes_per_line % 4 != 0);
  dwords_per_line.checked_mul(4).ok_or(ImagineError::Value)
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
) -> Result<crate::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32_Sfloat>,
{
  use alloc::vec::Vec;

  let header = bmp_get_nice_header(bytes)?;
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut bitmap: crate::Bitmap<P> = {
    let mut pixels = Vec::new();
    pixels.try_reserve(target_pixel_count)?;
    crate::Bitmap { width: header.width, height: header.height, pixels }
  };
  let width = header.width;
  let data_span = header.data_span;
  let image_bytes = bytes.get(data_span.0..data_span.1).ok_or(ImagineError::Value)?;
  match header.data_format {
    BmpDataFormat::Indexed1 { palette_span }
    | BmpDataFormat::Indexed4 { palette_span }
    | BmpDataFormat::Indexed4Rle { palette_span }
    | BmpDataFormat::Indexed8 { palette_span }
    | BmpDataFormat::Indexed8Rle { palette_span } => {
      // If we make a 256 element palette then indexing into the palette with a u8
      // will tend to optimize away the bounds check, and it usually goes much
      // faster than using `.get(i).unwrap_or_default()` or similar.
      let mut palette: [P; 256] = [r32g32b32_Sfloat::BLACK.into(); 256];
      let pal_bytes = bytes.get(palette_span.0..palette_span.1).ok_or(ImagineError::Value)?;
      for (chunk, p) in pal_bytes.chunks_exact(4).zip(palette.iter_mut()) {
        *p = P::from(r32g32b32_Sfloat::from(r8g8b8_Srgb { b: chunk[0], g: chunk[1], r: chunk[2] }));
      }
      let black: P = P::from(r32g32b32_Sfloat::BLACK);
      bitmap.pixels.resize(target_pixel_count, black);
      match header.data_format {
        BmpDataFormat::Indexed4Rle { .. } => {
          let mut x: u32 = 0;
          let mut y: u32 = 0;
          'rle_for: for rle_op in bmp_iter_rle4(image_bytes) {
            match rle_op {
              BmpRle4Op::EndOfBmp => break 'rle_for,
              BmpRle4Op::Newline => {
                x = 0;
                y = y.wrapping_add(1);
              }
              BmpRle4Op::Run { count, index_h, index_l } => {
                let mut it = [index_h, index_l].into_iter().cycle();
                for _ in 0..count.get() {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(it.next().unwrap())]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Delta { right, up } => {
                x = x.wrapping_add(right);
                y = y.wrapping_add(up);
              }
              BmpRle4Op::Raw4 { a, b, c, d } => {
                for val in [a, b, c, d] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw3 { a, b, c } => {
                for val in [a, b, c] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw2 { a, b } => {
                for val in [a, b] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw1 { a } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(a)]
                }
                x = x.wrapping_add(1);
              }
            }
          }
        }
        BmpDataFormat::Indexed8Rle { .. } => {
          let mut x: u32 = 0;
          let mut y: u32 = 0;
          'rle_for: for rle_op in bmp_iter_rle8(image_bytes) {
            match rle_op {
              BmpRle8Op::EndOfBmp => break 'rle_for,
              BmpRle8Op::Newline => {
                x = 0;
                y = y.wrapping_add(1);
              }
              BmpRle8Op::Run { count, index } => {
                let color = palette[usize::from(index)];
                for _ in 0..count.get() {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = color;
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle8Op::Delta { right, up } => {
                x = x.wrapping_add(right);
                y = y.wrapping_add(up);
              }
              BmpRle8Op::Raw2 { q, w } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(q)]
                }
                x = x.wrapping_add(1);
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(w)]
                }
                x = x.wrapping_add(1);
              }
              BmpRle8Op::Raw1 { q } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(q)]
                }
                x = x.wrapping_add(1);
              }
            }
          }
        }
        other => {
          let bits_per_pixel = match other {
            BmpDataFormat::Indexed1 { .. } => 1,
            BmpDataFormat::Indexed4 { .. } => 4,
            BmpDataFormat::Indexed8 { .. } => 8,
            _ => 8,
          };
          bitmap.pixels.extend(
            bmp_iter_pal_indexes_no_compression(image_bytes, width, bits_per_pixel)
              .map(|i| palette[usize::from(i)]),
          )
        }
      }
    }
    BmpDataFormat::BGR24 => bitmap.pixels.extend(
      bmp_iter_bgr24(image_bytes, width)
        .map(|[b, g, r]| P::from(r32g32b32_Sfloat::from(r8g8b8_Srgb { r, g, b }))),
    ),
    BmpDataFormat::Bitmask16RGB { r_mask, g_mask, b_mask } => {
      bitmap
        .pixels
        .extend(bmp_iter_bitmask16_rgb(image_bytes, r_mask, g_mask, b_mask, width).map(P::from));
    }
    BmpDataFormat::Bitmask32RGB { r_mask, g_mask, b_mask } => {
      if r_mask.count_ones() == 8 && g_mask.count_ones() == 8 && b_mask.count_ones() == 8 {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_srgb(image_bytes, r_mask, g_mask, b_mask, width)
            .map(|srgb| P::from(r32g32b32_Sfloat::from(srgb))),
        );
      } else {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_linear_rgb(image_bytes, r_mask, g_mask, b_mask, width).map(P::from),
        );
      }
    }
    _ => return Err(ImagineError::Value),
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
) -> Result<crate::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32a32_Sfloat>,
{
  use alloc::vec::Vec;

  let header = bmp_get_nice_header(bytes)?;
  if header.width > 17_000 || header.height > 17_000 {
    return Err(ImagineError::DimensionsTooLarge);
  }
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut bitmap: crate::Bitmap<P> = {
    let mut pixels = Vec::new();
    pixels.try_reserve(target_pixel_count)?;
    crate::Bitmap { width: header.width, height: header.height, pixels }
  };
  let width = header.width;
  let data_span = header.data_span;
  let image_bytes = bytes.get(data_span.0..data_span.1).ok_or(ImagineError::Value)?;
  match header.data_format {
    BmpDataFormat::Indexed1 { palette_span }
    | BmpDataFormat::Indexed4 { palette_span }
    | BmpDataFormat::Indexed4Rle { palette_span }
    | BmpDataFormat::Indexed8 { palette_span }
    | BmpDataFormat::Indexed8Rle { palette_span } => {
      // If we make a 256 element palette then indexing into the palette with a u8
      // will tend to optimize away the bounds check, and it usually goes much
      // faster than using `.get(i).unwrap_or_default()` or similar.
      let mut palette: [P; 256] = [r32g32b32a32_Sfloat::TRANSPARENT_BLACK.into(); 256];
      let pal_bytes = bytes.get(palette_span.0..palette_span.1).ok_or(ImagineError::Value)?;
      for (chunk, p) in pal_bytes.chunks_exact(4).zip(palette.iter_mut()) {
        *p = P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb {
          b: chunk[0],
          g: chunk[1],
          r: chunk[2],
          a: u8::MAX,
        }));
      }
      match header.data_format {
        BmpDataFormat::Indexed4Rle { .. } => {
          let black: P = P::from(r32g32b32a32_Sfloat::TRANSPARENT_BLACK);
          bitmap.pixels.resize(target_pixel_count, black);
          let mut x: u32 = 0;
          let mut y: u32 = 0;
          'rle_for: for rle_op in bmp_iter_rle4(image_bytes) {
            match rle_op {
              BmpRle4Op::EndOfBmp => break 'rle_for,
              BmpRle4Op::Newline => {
                x = 0;
                y = y.wrapping_add(1);
              }
              BmpRle4Op::Run { count, index_h, index_l } => {
                let mut it = [index_h, index_l].into_iter().cycle();
                for _ in 0..count.get() {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(it.next().unwrap())]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Delta { right, up } => {
                x = x.wrapping_add(right);
                y = y.wrapping_add(up);
              }
              BmpRle4Op::Raw4 { a, b, c, d } => {
                for val in [a, b, c, d] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw3 { a, b, c } => {
                for val in [a, b, c] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw2 { a, b } => {
                for val in [a, b] {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = palette[usize::from(val)]
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle4Op::Raw1 { a } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(a)]
                }
                x = x.wrapping_add(1);
              }
            }
          }
        }
        BmpDataFormat::Indexed8Rle { .. } => {
          let black: P = P::from(r32g32b32a32_Sfloat::TRANSPARENT_BLACK);
          bitmap.pixels.resize(target_pixel_count, black);
          let mut x: u32 = 0;
          let mut y: u32 = 0;
          'rle_for: for rle_op in bmp_iter_rle8(image_bytes) {
            match rle_op {
              BmpRle8Op::EndOfBmp => break 'rle_for,
              BmpRle8Op::Newline => {
                x = 0;
                y = y.wrapping_add(1);
              }
              BmpRle8Op::Run { count, index } => {
                let color = palette[usize::from(index)];
                for _ in 0..count.get() {
                  if let Some(p) = bitmap.get_mut(x, y) {
                    *p = color;
                  }
                  x = x.wrapping_add(1);
                }
              }
              BmpRle8Op::Delta { right, up } => {
                x = x.wrapping_add(right);
                y = y.wrapping_add(up);
              }
              BmpRle8Op::Raw2 { q, w } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(q)]
                }
                x = x.wrapping_add(1);
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(w)]
                }
                x = x.wrapping_add(1);
              }
              BmpRle8Op::Raw1 { q } => {
                if let Some(p) = bitmap.get_mut(x, y) {
                  *p = palette[usize::from(q)]
                }
                x = x.wrapping_add(1);
              }
            }
          }
        }
        other => {
          let bits_per_pixel = match other {
            BmpDataFormat::Indexed1 { .. } => 1,
            BmpDataFormat::Indexed4 { .. } => 4,
            BmpDataFormat::Indexed8 { .. } => 8,
            _ => 8,
          };
          bitmap.pixels.extend(
            bmp_iter_pal_indexes_no_compression(image_bytes, width, bits_per_pixel)
              .map(|i| palette[usize::from(i)]),
          )
        }
      }
    }
    BmpDataFormat::BGR24 => bitmap
      .pixels
      .extend(bmp_iter_bgr24(image_bytes, width).map(|[b, g, r]| {
        P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a: u8::MAX }))
      })),
    BmpDataFormat::Bitmask16RGB { r_mask, g_mask, b_mask } => {
      bitmap.pixels.extend(
        bmp_iter_bitmask16_rgb(image_bytes, r_mask, g_mask, b_mask, width)
          .map(|rgb| P::from(r32g32b32a32_Sfloat::from(rgb))),
      );
    }
    BmpDataFormat::Bitmask32RGB { r_mask, g_mask, b_mask } => {
      if r_mask.count_ones() == 8 && g_mask.count_ones() == 8 && b_mask.count_ones() == 8 {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_srgb(image_bytes, r_mask, g_mask, b_mask, width)
            .map(|srgb| P::from(r32g32b32a32_Sfloat::from(r32g32b32_Sfloat::from(srgb)))),
        );
      } else {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_linear_rgb(image_bytes, r_mask, g_mask, b_mask, width)
            .map(|rgb| P::from(r32g32b32a32_Sfloat::from(rgb))),
        );
      }
    }
    BmpDataFormat::Bitmask16RGBA { r_mask, g_mask, b_mask, a_mask } => {
      bitmap.pixels.extend(
        bmp_iter_bitmask16_rgba(image_bytes, r_mask, g_mask, b_mask, a_mask, width).map(P::from),
      );
    }
    BmpDataFormat::Bitmask32RGBA { r_mask, g_mask, b_mask, a_mask } => {
      if r_mask.count_ones() == 8 && g_mask.count_ones() == 8 && b_mask.count_ones() == 8 {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_srgba(image_bytes, r_mask, g_mask, b_mask, a_mask, width)
            .map(|srgb| P::from(r32g32b32a32_Sfloat::from(srgb))),
        );
      } else {
        bitmap.pixels.extend(
          bmp_iter_bitmask32_linear_rgba(image_bytes, r_mask, g_mask, b_mask, a_mask, width)
            .map(P::from),
        );
      }
    }
  }
  let black: P = P::from(r32g32b32a32_Sfloat::TRANSPARENT_BLACK);
  bitmap.pixels.resize(target_pixel_count, black);
  if header.origin_top_left != origin_top_left {
    bitmap.vertical_flip();
  }
  Ok(bitmap)
}
