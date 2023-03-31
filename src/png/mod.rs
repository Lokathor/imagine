//! Module for working with PNG data.
//!
//! * [Portable Network Graphics Specification (Second Edition)][png-spec]
//!
//! [png-spec]: https://www.w3.org/TR/2003/REC-PNG-20031110/

use core::fmt::{Debug, Write};

use bitfrob::u8_replicate_bits;
use pixel_formats::{r8g8b8_Unorm, r8g8b8a8_Unorm};

use crate::sRGBIntent;

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

/// Gets the palette out of the PNG bytes.
///
/// Each `[u8;3]` in the palette is an `[r8, g8, b8]` color entry.
#[inline]
pub fn png_get_palette(bytes: &[u8]) -> Option<&[r8g8b8_Unorm]> {
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

#[cfg(all(feature = "alloc", feature = "miniz_oxide"))]
impl<P> crate::image::Bitmap<P>
where
  P: From<r8g8b8a8_Unorm> + Clone,
{
  /// Attempts to make a [Bitmap](crate::image::Bitmap) from PNG bytes.
  ///
  /// ## Failure
  /// Errors include, but are not limited to:
  /// * No [IHDR] found in the bytes.
  /// * Allocation failure.
  ///
  /// There's currently no specific error reported, you just get `None`.
  #[cfg_attr(docs_rs, doc(cfg(all(feature = "png", feature = "miniz_oxide"))))]
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn try_from_png_bytes(bytes: &[u8]) -> Option<Self> {
    use alloc::vec::Vec;
    //
    let ihdr = match png_get_header(bytes) {
      Some(ihdr) => ihdr,
      None => {
        // No Image Header prevents all further processing.
        return None;
      }
    };
    let zlib_len = ihdr.get_zlib_decompression_requirement();
    let mut zlib_buffer: Vec<u8> = Vec::new();
    zlib_buffer.try_reserve(zlib_len).ok()?;
    // ferris plz make this into a memset
    zlib_buffer.resize(zlib_len, 0);
    match miniz_oxide::inflate::decompress_slice_iter_to_slice(
      &mut zlib_buffer,
      png_get_idat(bytes),
      true,
      true,
    ) {
      Ok(decompression_count) => {
        if decompression_count < zlib_buffer.len() {
          // potential a cause for concern, but i guess keep going. We already
          // put enough zeros into the zlib buffer so we'll be able to unfilter
          // either way.
        }
      }
      Err(_e) => {
        // should we cancel with an error? The most likely errors are that
        // there's not actually Zlib compressed data (so we'd have an image of
        // zeros) or there's too much Zlib compressed data (in which case we
        // can maybe proceed with partial results).
      }
    }
    let pixel_count = ihdr.width.checked_mul(ihdr.height)? as usize;
    let mut pixels: Vec<P> = Vec::new();
    pixels.try_reserve(pixel_count).ok()?;
    // ferris plz make this into a memset
    pixels.resize(pixel_count, r8g8b8a8_Unorm::default().into());
    let mut image = Self { width: ihdr.width, height: ihdr.height, pixels };
    let plte: &[r8g8b8_Unorm] = if ihdr.color_type == PngColorType::Index {
      png_get_palette(bytes).unwrap_or(&[])
    } else {
      &[]
    };
    let (trns_y, trns_rgb, trns_alphas): (Option<u16>, Option<[u16; 3]>, Option<&[u8]>) = {
      if let Some(trns) = png_get_transparency(bytes) {
        (trns.try_to_grayscale(), trns.try_to_rgb(), Some(trns.to_alphas()))
      } else {
        (None, None, None)
      }
    };
    let unfilter_op = |x: u32, y: u32, data: &[u8]| {
      if let Some(p) = image.get_mut(x, y) {
        match ihdr.color_type {
          PngColorType::RGB => {
            let [r, g, b] = if ihdr.bit_depth == 16 {
              [data[0], data[2], data[4]]
            } else {
              [data[0], data[1], data[2]]
            };
            let full = if ihdr.bit_depth == 16 {
              Some([
                u16::from_be_bytes([data[0], data[1]]),
                u16::from_be_bytes([data[2], data[3]]),
                u16::from_be_bytes([data[4], data[5]]),
              ])
            } else {
              Some([data[0] as u16, data[1] as u16, data[2] as u16])
            };
            let a = if trns_rgb == full { 0 } else { 255 };
            *p = r8g8b8a8_Unorm { r, g, b, a }.into();
          }
          PngColorType::RGBA => {
            let [r, g, b, a] = if ihdr.bit_depth == 16 {
              [data[0], data[2], data[4], data[6]]
            } else {
              [data[0], data[1], data[2], data[3]]
            };
            *p = r8g8b8a8_Unorm { r, g, b, a }.into();
          }
          PngColorType::YA => {
            let [y, a] = if ihdr.bit_depth == 16 { [data[0], data[2]] } else { [data[0], data[1]] };
            *p = r8g8b8a8_Unorm { r: y, g: y, b: y, a }.into();
          }
          PngColorType::Y => {
            let y = if ihdr.bit_depth == 16 {
              data[0]
            } else {
              u8_replicate_bits(ihdr.bit_depth as u32, data[0])
            };
            let full = if ihdr.bit_depth == 16 {
              Some(u16::from_be_bytes([data[0], data[1]]))
            } else {
              Some(data[0] as u16)
            };
            let a = if trns_y == full { 0 } else { 255 };
            *p = r8g8b8a8_Unorm { r: y, g: y, b: y, a }.into();
          }
          PngColorType::Index => {
            let r8g8b8_Unorm { r, g, b } =
              *plte.get(data[0] as usize).unwrap_or(&r8g8b8_Unorm::default());
            let a = if let Some(alphas) = trns_alphas {
              *alphas.get(data[0] as usize).unwrap_or(&255)
            } else {
              255
            };
            *p = r8g8b8a8_Unorm { r, g, b, a }.into()
          }
        }
      } else {
        // attempted out of bounds pixel write, but i guess it doesn't matter?
      }
    };
    if let Err(_e) = ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op) {
      // err during unfiltering, do we care?
    }
    Some(image)
  }
}

#[cfg(all(feature = "alloc", feature = "miniz_oxide"))]
impl crate::image::Palmap<u8, r8g8b8a8_Unorm> {
  /// Attempts to make a [Palmap](crate::image::Palmap) from PNG bytes.
  ///
  /// ## Failure
  /// Errors include, but are not limited to:
  /// * No [IHDR] found in the bytes.
  /// * The PNG's "color type" field (in the header) is not Index color.
  /// * Allocation failure.
  ///
  /// There's currently no specific error reported, you just get `None`.
  #[cfg_attr(docs_rs, doc(cfg(all(feature = "png", feature = "miniz_oxide"))))]
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn try_from_png_bytes(bytes: &[u8]) -> Option<Self> {
    use alloc::vec::Vec;
    //
    let ihdr = match png_get_header(bytes) {
      Some(ihdr) => ihdr,
      None => {
        // No Image Header prevents all further processing.
        return None;
      }
    };
    if ihdr.color_type != PngColorType::Index {
      return None;
    }
    let zlib_len = ihdr.get_zlib_decompression_requirement();
    let mut zlib_buffer: Vec<u8> = Vec::new();
    zlib_buffer.try_reserve(zlib_len).ok()?;
    // ferris plz make this into a memset
    zlib_buffer.resize(zlib_len, 0);
    match miniz_oxide::inflate::decompress_slice_iter_to_slice(
      &mut zlib_buffer,
      png_get_idat(bytes),
      true,
      true,
    ) {
      Ok(decompression_count) => {
        if decompression_count < zlib_buffer.len() {
          // potential a cause for concern, but i guess keep going. We already
          // put enough zeros into the zlib buffer so we'll be able to unfilter
          // either way.
        }
      }
      Err(_e) => {
        // should we cancel with an error? The most likely errors are that
        // there's not actually Zlib compressed data (so we'd have an image of
        // zeros) or there's too much Zlib compressed data (in which case we
        // can maybe proceed with partial results).
      }
    }
    let pixel_count = (ihdr.width * ihdr.height) as usize;
    let mut indexes: Vec<u8> = Vec::new();
    indexes.try_reserve(pixel_count).ok()?;
    // ferris plz make this into a memset
    indexes.resize(pixel_count, 0_u8);
    let mut palette: Vec<r8g8b8a8_Unorm> = match png_get_palette(bytes) {
      Some(pal_slice) => pal_slice
        .iter()
        .copied()
        .map(|r8g8b8_Unorm { r, g, b }| r8g8b8a8_Unorm { r, g, b, a: 255 })
        .collect(),
      None => return None,
    };
    if let Some(tRNS(bytes)) = png_get_transparency(bytes) {
      palette.iter_mut().zip(bytes.iter()).for_each(|(p, b)| p.a = *b)
    }
    let mut palmap = Self { width: ihdr.width, height: ihdr.height, indexes, palette };
    let unfilter_op = |x: u32, y: u32, data: &[u8]| {
      if let Some(i) = palmap.get_mut(x, y) {
        *i = data[0];
      } else {
        // attempted out of bounds pixel write, but i guess it doesn't matter?
      }
    };
    if let Err(_e) = ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op) {
      // err during unfiltering, do we care?
    }
    Some(palmap)
  }
}
