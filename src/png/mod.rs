//! Module for working with PNG data.
//!
//! * [Portable Network Graphics Specification (Second Edition)][png-spec]
//!
//! [png-spec]: https://www.w3.org/TR/2003/REC-PNG-20031110/

use crate::{sRGBIntent, ImagineError};
use bitfrob::u8_replicate_bits;
use core::fmt::{Debug, Write};
use pixel_formats::{r32g32b32a32_Sfloat, r8g8b8_Unorm, r8g8b8a8_Unorm};

// TODO: CRC support for raw chunks is needed later to write PNG data.

mod tests;

mod bkgd;
mod idat;
mod ihdr;
mod plte;
mod png_chunk;
mod raw_chunk;
mod trns;

pub use self::{bkgd::*, idat::*, ihdr::*, plte::*, png_chunk::*, raw_chunk::*, trns::*};

/// Checks if the PNG's initial 8 bytes are correct.
#[inline]
pub const fn png_signature_is_correct(bytes: &[u8]) -> bool {
  matches!(bytes, [137, 80, 78, 71, 13, 10, 26, 10, ..])
}

/// Gets the [IHDR] out of the PNG bytes.
#[inline]
pub fn png_get_header(bytes: &[u8]) -> Option<IHDR> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    IHDR::try_from(png_chunk).ok()
  })
}

/// Gets the transparency chunk for the PNG bytes, if any.
#[inline]
pub fn png_get_transparency(bytes: &[u8]) -> Option<tRNS<'_>> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let trns = tRNS::try_from(png_chunk).ok()?;
    Some(trns)
  })
}

/// Gets the background color information, if any.
#[inline]
pub fn png_get_background_color(bytes: &[u8]) -> Option<bKGD> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let bkgd = bKGD::try_from(png_chunk).ok()?;
    Some(bkgd)
  })
}

/// Gets the sRGB info in the PNG, if any
#[inline]
pub fn png_get_srgb(bytes: &[u8]) -> Option<sRGBIntent> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    match png_chunk {
      PngChunk::sRGB(srgb) => Some(srgb),
      _ => None,
    }
  })
}

/// Gets the gamma info in the PNG, if any
#[inline]
pub fn png_get_gamma(bytes: &[u8]) -> Option<u32> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    match png_chunk {
      PngChunk::gAMA(g) => Some(g),
      _ => None,
    }
  })
}

/// Gets the palette out of the PNG bytes.
///
/// Each `[u8;3]` in the palette is an `[r8, g8, b8]` color entry.
#[inline]
pub fn png_get_palette(bytes: &[u8]) -> Option<&[[u8; 3]]> {
  PngRawChunkIter::new(bytes).find_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let plte = PLTE::try_from(png_chunk).ok()?;
    Some(plte.entries())
  })
}

/// Gets an iterator over all the [IDAT] slices in the PNG bytes.
#[inline]
pub fn png_get_idat(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
  PngRawChunkIter::new(bytes).filter_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let idat = IDAT::try_from(png_chunk).ok()?;
    Some(idat.as_bytes())
  })
}

/// Given the dimensions of the full PNG image, computes the size of each
/// reduced image.
///
/// The PNG interlacing scheme converts a full image to 7 reduced images, each
/// with potentially separate dimensions. Knowing the size of each reduced image
/// is important for the unfiltering process.
///
/// The output uses index 0 as the base image size, and indexes 1 through 7 for
/// the size of reduced images 1 through 7.
///
/// PS: Interlacing is terrible, don't interlace your images.
#[inline]
#[must_use]
const fn reduced_image_dimensions(full_width: u32, full_height: u32) -> [(u32, u32); 8] {
  // ```
  // 1 6 4 6 2 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // 3 6 4 6 3 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // ```
  let grids_w = full_width / 8;
  let grids_h = full_height / 8;
  //
  let partial_w = full_width % 8;
  let partial_h = full_height % 8;
  //
  let zero = (full_width, full_height);
  //
  let first = (grids_w + (partial_w + 7) / 8, grids_h + (partial_h + 7) / 8);
  let second = (grids_w + (partial_w + 3) / 8, grids_h + (partial_h + 7) / 8);
  let third = (grids_w * 2 + ((partial_w + 3) / 4), grids_h + ((partial_h + 3) / 8));
  let fourth = (grids_w * 2 + (partial_w + 1) / 4, grids_h * 2 + (partial_h + 3) / 4);
  let fifth = (grids_w * 4 + ((partial_w + 1) / 2), grids_h * 2 + (partial_h + 1) / 4);
  let sixth = (grids_w * 4 + partial_w / 2, grids_h * 4 + ((partial_h + 1) / 2));
  let seventh = (grids_w * 8 + partial_w, grids_h * 4 + (partial_h / 2));
  //
  [zero, first, second, third, fourth, fifth, sixth, seventh]
}

/// Converts a reduced image location into the full image location.
///
/// For consistency between this function and the [reduced_image_dimensions]
/// function, when giving an `image_level` of 0 the output will be the same as
/// the input.
///
/// ## Panics
/// * If the image level given exceeds 7.
#[inline]
#[must_use]
const fn interlaced_pos_to_full_pos(
  image_level: usize, reduced_x: u32, reduced_y: u32,
) -> (u32, u32) {
  // ```
  // 1 6 4 6 2 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // 3 6 4 6 3 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // ```
  #[allow(clippy::identity_op)]
  match image_level {
    0 /* full image */ => (reduced_x, reduced_y),
    1 => (reduced_x * 8 + 0, reduced_y * 8 + 0),
    2 => (reduced_x * 8 + 4, reduced_y * 8 + 0),
    3 => (reduced_x * 4 + 0, reduced_y * 8 + 4),
    4 => (reduced_x * 4 + 2, reduced_y * 4 + 0),
    5 => (reduced_x * 2 + 0, reduced_y * 4 + 2),
    6 => (reduced_x * 2 + 1, reduced_y * 2 + 0),
    7 => (reduced_x * 1 + 0, reduced_y * 2 + 1),
    _ => panic!("reduced image level must be 1 through 7")
  }
}

/// Automatically allocate and fill in a [Bitmap](crate::image::Bitmap).
///
/// The output is automatically flipped as necessary so that the output will be
/// oriented with the origin in the top left.
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn png_try_bitmap_rgba<P>(
  bytes: &[u8], origin_top_left: bool,
) -> Result<crate::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32a32_Sfloat> + core::fmt::Debug,
{
  use alloc::vec::Vec;
  use bytemuck::cast_slice;
  use pixel_formats::{r8g8b8_Srgb, r8g8b8a8_Srgb};

  for (n, raw_chunk) in PngRawChunkIter::new(bytes).enumerate() {
    let chunk_res = PngChunk::try_from(raw_chunk);
    println!("{n}: {chunk_res:?}");
  }

  let ihdr = png_get_header(bytes).ok_or(ImagineError::Parse)?;
  if ihdr.width > 17_000 || ihdr.height > 17_000 {
    return Err(ImagineError::DimensionsTooLarge);
  }

  let transparent_black: P = P::from(r32g32b32a32_Sfloat::TRANSPARENT_BLACK);
  let target_pixel_count: usize =
    ihdr.width.checked_mul(ihdr.height).ok_or(ImagineError::Value)?.try_into()?;
  let mut bitmap: crate::Bitmap<P> = {
    let mut pixels = Vec::new();
    pixels.try_reserve(target_pixel_count)?;
    pixels.resize(target_pixel_count, transparent_black);
    crate::Bitmap { width: ihdr.width, height: ihdr.height, pixels }
  };

  let mut zlib_buffer: Vec<u8> = {
    let zlib_len = ihdr.get_zlib_decompression_requirement();
    let mut zlib_buffer: Vec<u8> = Vec::new();
    zlib_buffer.try_reserve(zlib_len)?;
    zlib_buffer.resize(zlib_len, 0);
    zlib_buffer
  };
  let _who_cares = miniz_oxide::inflate::decompress_slice_iter_to_slice(
    &mut zlib_buffer,
    png_get_idat(bytes),
    true,
    true,
  );

  let is_srgb = png_get_srgb(bytes).is_some();

  let gamma = png_get_gamma(bytes).unwrap_or(100_000_u32) as f32 / 100_000.0_f32;
  let gamma_exp = 1.0 / gamma;
  println!("{gamma_exp}");

  let trns: Option<tRNS<'_>> = png_get_transparency(bytes);
  let trns_y = trns.and_then(|trns| trns.try_to_grayscale());
  let trns_rgb = trns.and_then(|trns| trns.try_to_rgb());

  let mut palette: [P; 256] = [r32g32b32a32_Sfloat::TRANSPARENT_BLACK.into(); 256];
  if ihdr.color_type == PngColorType::Index {
    if is_srgb {
      let plte: &[r8g8b8_Srgb] = cast_slice(png_get_palette(bytes).unwrap_or(&[]));
      let trns: &[u8] = trns.map(|trns| trns.to_alphas()).unwrap_or(&[]);
      palette.iter_mut().zip(plte.iter().copied()).enumerate().for_each(
        |(i, (palette, r8g8b8_Srgb { r, g, b }))| {
          let a: u8 = trns.get(i).copied().unwrap_or(u8::MAX);
          let gamma_corrected = r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a });
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *palette = P::from(pre_multiplied_alpha);
        },
      );
    } else {
      let plte: &[r8g8b8_Unorm] = cast_slice(png_get_palette(bytes).unwrap_or(&[]));
      let trns: &[u8] = trns.map(|trns| trns.to_alphas()).unwrap_or(&[]);
      palette.iter_mut().zip(plte.iter().copied()).enumerate().for_each(
        |(i, (palette, r8g8b8_Unorm { r, g, b }))| {
          let a: u8 = trns.get(i).copied().unwrap_or(u8::MAX);
          let unorm = r8g8b8a8_Unorm { r, g, b, a };
          let sfloat = r32g32b32a32_Sfloat::from(unorm);
          let gamma_corrected = r32g32b32a32_Sfloat {
            r: sfloat.r.powf(gamma_exp),
            g: sfloat.g.powf(gamma_exp),
            b: sfloat.b.powf(gamma_exp),
            a: sfloat.a,
          };
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *palette = P::from(pre_multiplied_alpha);
        },
      );
    }
  };

  match ihdr.color_type {
    PngColorType::Index => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          *p = palette[usize::from(data[0])];
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::Y if ihdr.bit_depth == 16 => {
      // depth 16 needs separate handling from 8 or less.
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let y = u16::from_be_bytes([data[0], data[1]]);
          *p = if Some(y) == trns_y {
            transparent_black
          } else {
            let y = (y as f32) / (u16::MAX as f32);
            let sfloat = r32g32b32a32_Sfloat { r: y, g: y, b: y, a: 1.0 };
            let gamma_corrected = r32g32b32a32_Sfloat {
              r: sfloat.r.powf(gamma_exp),
              g: sfloat.g.powf(gamma_exp),
              b: sfloat.b.powf(gamma_exp),
              a: sfloat.a,
            };
            let pre_multiplied_alpha = r32g32b32a32_Sfloat {
              r: gamma_corrected.r * gamma_corrected.a,
              g: gamma_corrected.g * gamma_corrected.a,
              b: gamma_corrected.b * gamma_corrected.a,
              a: gamma_corrected.a,
            };
            P::from(pre_multiplied_alpha)
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::Y if is_srgb => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          *p = if Some(u16::from(data[0])) == trns_y {
            transparent_black
          } else {
            let y = u8_replicate_bits(u32::from(ihdr.bit_depth), data[0]);
            P::from(r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r: y, g: y, b: y, a: u8::MAX }))
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::Y => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          *p = if Some(u16::from(data[0])) == trns_y {
            transparent_black
          } else {
            let y = u8_replicate_bits(u32::from(ihdr.bit_depth), data[0]);
            let y = (y as f32) / (u8::MAX as f32);
            let sfloat = r32g32b32a32_Sfloat { r: y, g: y, b: y, a: 1.0 };
            let gamma_corrected = r32g32b32a32_Sfloat {
              r: sfloat.r.powf(gamma_exp),
              g: sfloat.g.powf(gamma_exp),
              b: sfloat.b.powf(gamma_exp),
              a: sfloat.a,
            };
            // no alpha multiply, alpha is known 1.0
            P::from(gamma_corrected)
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::YA if ihdr.bit_depth == 16 => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let y = (u16::from_be_bytes([data[0], data[1]]) as f32) / (u16::MAX as f32);
          let a = (u16::from_be_bytes([data[2], data[3]]) as f32) / (u16::MAX as f32);
          let sfloat = r32g32b32a32_Sfloat { r: y, g: y, b: y, a };
          let gamma_corrected = r32g32b32a32_Sfloat {
            r: sfloat.r.powf(gamma_exp),
            g: sfloat.g.powf(gamma_exp),
            b: sfloat.b.powf(gamma_exp),
            a: sfloat.a,
          };
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::YA if is_srgb => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let y = data[0];
          let a = data[1];
          let gamma_corrected = r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r: y, g: y, b: y, a });
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::YA => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let y = (data[0] as f32) / (u8::MAX as f32);
          let a = (data[1] as f32) / (u8::MAX as f32);
          let sfloat = r32g32b32a32_Sfloat { r: y, g: y, b: y, a };
          let gamma_corrected = r32g32b32a32_Sfloat {
            r: sfloat.r.powf(gamma_exp),
            g: sfloat.g.powf(gamma_exp),
            b: sfloat.b.powf(gamma_exp),
            a: sfloat.a,
          };
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGB if ihdr.bit_depth == 16 => {
      // depth 16 needs separate handling from 8 or less.
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = u16::from_be_bytes([data[0], data[1]]);
          let g = u16::from_be_bytes([data[2], data[3]]);
          let b = u16::from_be_bytes([data[4], data[5]]);
          *p = if Some([r, g, b]) == trns_rgb {
            transparent_black
          } else {
            let r = (u16::from_be_bytes([data[0], data[1]]) as f32) / (u16::MAX as f32);
            let g = (u16::from_be_bytes([data[2], data[3]]) as f32) / (u16::MAX as f32);
            let b = (u16::from_be_bytes([data[4], data[5]]) as f32) / (u16::MAX as f32);
            let sfloat = r32g32b32a32_Sfloat { r, g, b, a: 1.0 };
            let gamma_corrected = r32g32b32a32_Sfloat {
              r: sfloat.r.powf(gamma_exp),
              g: sfloat.g.powf(gamma_exp),
              b: sfloat.b.powf(gamma_exp),
              a: sfloat.a,
            };
            // no alpha multiply, alpha is known 1.0
            P::from(gamma_corrected)
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGB if is_srgb => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = data[0];
          let g = data[1];
          let b = data[2];
          *p = if Some([u16::from(r), u16::from(g), u16::from(b)]) == trns_rgb {
            transparent_black
          } else {
            let gamma_corrected = r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a: u8::MAX });
            // no alpha multiply, alpha is known 1.0
            P::from(gamma_corrected)
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGB => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = data[0];
          let g = data[1];
          let b = data[2];
          *p = if Some([u16::from(r), u16::from(g), u16::from(b)]) == trns_rgb {
            transparent_black
          } else {
            let r = (data[0] as f32) / (u8::MAX as f32);
            let g = (data[1] as f32) / (u8::MAX as f32);
            let b = (data[2] as f32) / (u8::MAX as f32);
            let sfloat = r32g32b32a32_Sfloat { r, g, b, a: 1.0 };
            let gamma_corrected = r32g32b32a32_Sfloat {
              r: sfloat.r.powf(gamma_exp),
              g: sfloat.g.powf(gamma_exp),
              b: sfloat.b.powf(gamma_exp),
              a: sfloat.a,
            };
            // no alpha multiply, alpha is known 1.0
            P::from(gamma_corrected)
          };
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGBA if ihdr.bit_depth == 16 => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = (u16::from_be_bytes([data[0], data[1]]) as f32) / (u16::MAX as f32);
          let g = (u16::from_be_bytes([data[2], data[3]]) as f32) / (u16::MAX as f32);
          let b = (u16::from_be_bytes([data[4], data[5]]) as f32) / (u16::MAX as f32);
          let a = (u16::from_be_bytes([data[6], data[7]]) as f32) / (u16::MAX as f32);
          let sfloat = r32g32b32a32_Sfloat { r, g, b, a };
          let gamma_corrected = r32g32b32a32_Sfloat {
            r: sfloat.r.powf(gamma_exp),
            g: sfloat.g.powf(gamma_exp),
            b: sfloat.b.powf(gamma_exp),
            a: sfloat.a,
          };
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGBA if is_srgb => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = data[0];
          let g = data[1];
          let b = data[2];
          let a = data[3];
          let gamma_corrected = r32g32b32a32_Sfloat::from(r8g8b8a8_Srgb { r, g, b, a });
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
    PngColorType::RGBA => {
      let unfilter_op = |x: u32, y: u32, data: &[u8]| {
        if let Some(p) = bitmap.get_mut(x, y) {
          let r = (data[0] as f32) / (u8::MAX as f32);
          let g = (data[1] as f32) / (u8::MAX as f32);
          let b = (data[2] as f32) / (u8::MAX as f32);
          let a = (data[3] as f32) / (u8::MAX as f32);
          let sfloat = r32g32b32a32_Sfloat { r, g, b, a };
          let gamma_corrected = r32g32b32a32_Sfloat {
            r: sfloat.r.powf(gamma_exp),
            g: sfloat.g.powf(gamma_exp),
            b: sfloat.b.powf(gamma_exp),
            a: sfloat.a,
          };
          let pre_multiplied_alpha = r32g32b32a32_Sfloat {
            r: gamma_corrected.r * gamma_corrected.a,
            g: gamma_corrected.g * gamma_corrected.a,
            b: gamma_corrected.b * gamma_corrected.a,
            a: gamma_corrected.a,
          };
          *p = P::from(pre_multiplied_alpha);
        }
      };
      ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op).ok();
    }
  }

  if !origin_top_left {
    bitmap.vertical_flip();
  }
  Ok(bitmap)
}
