#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

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
//!   is 8 (or less) bits per pixel (and `None` indicates `2**bits_per_pixel`
//!   entries), and otherwise it just suggests the colors that a limited-color
//!   display might want to favor (and `None` indicates 0 entries). Each entry
//!   in the color table is generally a `[u8;4]` value (`[r, g, b, a]`),
//!   **except** if `BmpInfoHeaderCore` is used, in which case each entry is a
//!   `[u8;3]` value (`[r, g, b]`). Usually all alpha values in the color table
//!   will be 0, the values are only 4 bytes each for alignment, but all colors
//!   are still supposed to be opaque (make appropriate adjustments). If a
//!   non-zero alpha value is found in the palette then the palette is probably
//!   alpha aware, and you should leave the alpha channels alone.
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

use crate::{sRGBIntent, util::*};
use bytemuck::cast;
use core::{
  fmt::Write,
  num::{NonZeroU16, NonZeroU32},
};
use pixel_formats::r8g8b8a8_Unorm;

mod colorspace;
mod file_header;
mod ih_core;
mod ih_os2;
mod ih_v1;
mod ih_v2;
mod ih_v3;
mod ih_v4;
mod ih_v5;
mod info_header;

pub use self::{
  colorspace::*, file_header::*, ih_core::*, ih_os2::*, ih_v1::*, ih_v2::*, ih_v3::*, ih_v4::*,
  ih_v5::*, info_header::*,
};

#[derive(Debug)]
pub struct BmpHeader {
  pub file_header: BmpFileHeader,
  pub info_header: BmpInfoHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum BmpError {
  ThisIsProbablyNotABmpFile,
  InsufficientBytes,
  IncorrectSizeForThisInfoHeaderVersion,
  UnknownCompression,
  UnknownHeaderLength,
  IllegalBitDepth,
  AllocError,
  PixelDataIllegalLength,
  PixelDataIllegalRLEContent,
  NotAPalettedBmp,
  WidthOrHeightZero,
  /// The BMP file might be valid, but either way this library doesn't currently
  /// know how to parse it.
  ParserIncomplete,
}

/// Various possible compression styles for Bmp files.
///
/// * Indexed color images *can* use 4 bit RLE, 8 bit RLE, or Huffman 1D.
/// * `BmpInfoHeaderOs22x` *can* use 24 bit RLE.
/// * 16bpp and 32bpp images are *always* stored uncompressed.
/// * Any other image can also be stored uncompressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BmpCompression {
  /// RGB, No compression.
  RgbNoCompression = 0,

  /// RGB, Run-length encoded, 8bpp
  RgbRLE8 = 1,

  /// RGB, Run-length encoded, 4bpp
  RgbRLE4 = 2,

  /// Meaning depends on header:
  /// * OS/2 2.x: Huffman 1D
  /// * InfoHeader: The image is not compressed, and following the header
  ///   there's **three** `u32` bitmasks that let you locate the R, G, and B
  ///   bits. This should only be used with 16 or 32 bits per pixel bitmaps.
  Bitfields = 3,

  /// Meaning depends on header:
  /// * OS/2 2.x: RLE24
  /// * InfoHeader v4+: A jpeg image
  Jpeg = 4,

  ///
  /// * InfoHeader v4+: A png image
  Png = 5,

  /// The image is not compressed, and following the header
  /// there's **four** `u32` bitmasks that let you locate the R, G, B, and A
  /// bits. This should only be used with 16 or 32 bits per pixel bitmaps.
  AlphaBitfields = 6,

  /// CMYK, No compression.
  CmykNoCompression = 11,

  /// CMYK, Run-length encoded, 8bpp
  CmykRLE8 = 12,

  /// CMYK, Run-length encoded, 4bpp
  CmykRLE4 = 13,
}
impl TryFrom<u32> for BmpCompression {
  type Error = BmpError;
  #[inline]
  fn try_from(value: u32) -> Result<Self, Self::Error> {
    use BmpCompression::*;
    Ok(match value {
      0 => RgbNoCompression,
      1 => RgbRLE8,
      2 => RgbRLE4,
      3 => Bitfields,
      4 => Jpeg,
      5 => Png,
      6 => AlphaBitfields,
      11 => CmykNoCompression,
      12 => CmykRLE8,
      13 => CmykRLE4,
      _ => return Err(BmpError::UnknownCompression),
    })
  }
}
impl From<BmpCompression> for u32 {
  #[inline]
  #[must_use]
  fn from(c: BmpCompression) -> Self {
    c as u32
  }
}

/// Honestly, I don't know what this is about, but it's in the BMP header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Halftoning {
  /// No halftoning, the most common style.
  NoHalftoning,

  /// [wikipedia](https://en.wikipedia.org/wiki/Error_diffusion)
  ErrorDiffusion {
    /// 0 indicates that the error is not diffused.
    damping_percentage: u32,
  },

  /// PANDA: Processing Algorithm for Noncoded Document Acquisition.
  Panda {
    x: u32,
    y: u32,
  },

  SuperCircle {
    x: u32,
    y: u32,
  },

  Unknown,
}
impl From<[u8; 10]> for Halftoning {
  #[inline]
  fn from(a: [u8; 10]) -> Self {
    use BmpError::*;
    match u16_le(&a[0..2]) {
      0 => Halftoning::NoHalftoning,
      1 => Halftoning::ErrorDiffusion { damping_percentage: u32_le(&a[2..6]) },
      2 => Halftoning::Panda { x: u32_le(&a[2..6]), y: u32_le(&a[6..10]) },
      3 => Halftoning::SuperCircle { x: u32_le(&a[2..6]), y: u32_le(&a[6..10]) },
      _ => Halftoning::Unknown,
    }
  }
}
impl From<Halftoning> for [u8; 10] {
  #[inline]
  fn from(h: Halftoning) -> Self {
    let mut a = [0; 10];
    match h {
      Halftoning::NoHalftoning | Halftoning::Unknown => (),
      Halftoning::ErrorDiffusion { damping_percentage } => {
        a[0..2].copy_from_slice(1_u16.to_le_bytes().as_slice());
        a[2..6].copy_from_slice(damping_percentage.to_le_bytes().as_slice());
      }
      Halftoning::Panda { x, y } => {
        a[0..2].copy_from_slice(2_u16.to_le_bytes().as_slice());
        a[2..6].copy_from_slice(x.to_le_bytes().as_slice());
        a[6..10].copy_from_slice(y.to_le_bytes().as_slice());
      }
      Halftoning::SuperCircle { x, y } => {
        a[0..2].copy_from_slice(3_u16.to_le_bytes().as_slice());
        a[2..6].copy_from_slice(x.to_le_bytes().as_slice());
        a[6..10].copy_from_slice(y.to_le_bytes().as_slice());
      }
    }
    a
  }
}

const LCS_CALIBRATED_RGB: u32 = 0x00000000;
const LCS_sRGB: u32 = 0x7352_4742;
/// This is b"Win " in little-endian, I'm not kidding.
const LCS_WINDOWS_COLOR_SPACE: u32 = 0x5769_6E20;
const PROFILE_LINKED: u32 = 0x4C49_4E4B;
const PROFILE_EMBEDDED: u32 = 0x4D42_4544;
const LCS_GM_ABS_COLORIMETRIC: u32 = 0x00000008;
const LCS_GM_BUSINESS: u32 = 0x00000001;
const LCS_GM_GRAPHICS: u32 = 0x00000002;
const LCS_GM_IMAGES: u32 = 0x00000004;

#[cfg(feature = "alloc")]
impl<P> crate::image::Bitmap<P>
where
  P: From<r8g8b8a8_Unorm> + Clone,
{
  /// Attempts to parse the bytes of a BMP file into a bitmap.
  ///
  /// ## Failure
  /// Errors include, but are not limited to:
  /// * Incorrect Bmp File Header.
  /// * Allocation failure.
  #[cfg_attr(docs_rs, doc(cfg(all(feature = "bmp", feature = "miniz_oxide"))))]
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn try_from_bmp_bytes(bytes: &[u8]) -> Result<Self, BmpError> {
    use crate::util::try_split_off_byte_array;
    use alloc::vec::Vec;
    use bytemuck::cast_slice;
    use core::mem::size_of;
    //
    let (file_header, rest) = BmpFileHeader::try_from_bytes(bytes)?;
    if file_header.total_file_size as usize != bytes.len()
      || !(COMMON_BMP_TAGS.contains(&file_header.tag))
    {
      return Err(BmpError::ThisIsProbablyNotABmpFile);
    }
    let (info_header, mut rest) = BmpInfoHeader::try_from_bytes(rest)?;
    let compression = info_header.compression();
    let bits_per_pixel = info_header.bits_per_pixel() as usize;
    let width: usize = info_header.width().unsigned_abs() as usize;
    let height: usize = info_header.height().unsigned_abs() as usize;
    let pixel_count: usize = width.saturating_mul(height);
    if width == 0 || height == 0 {
      return Err(BmpError::WidthOrHeightZero);
    }

    let [r_mask, g_mask, b_mask, a_mask] = match info_header {
      BmpInfoHeader::Core(_) | BmpInfoHeader::Os22x(_) | BmpInfoHeader::V1(_) => {
        match compression {
          BmpCompression::Bitfields => {
            const U32X3: usize = size_of::<u32>() * 3;
            let (a, new_rest) =
              try_split_off_byte_array::<U32X3>(rest).ok_or(BmpError::InsufficientBytes)?;
            rest = new_rest;
            [
              u32::from_le_bytes(a[0..4].try_into().unwrap()),
              u32::from_le_bytes(a[4..8].try_into().unwrap()),
              u32::from_le_bytes(a[8..12].try_into().unwrap()),
              0,
            ]
          }
          BmpCompression::AlphaBitfields => {
            const U32X4: usize = size_of::<u32>() * 4;
            let (a, new_rest) =
              try_split_off_byte_array::<U32X4>(rest).ok_or(BmpError::InsufficientBytes)?;
            rest = new_rest;
            [
              u32::from_le_bytes(a[0..4].try_into().unwrap()),
              u32::from_le_bytes(a[4..8].try_into().unwrap()),
              u32::from_le_bytes(a[8..12].try_into().unwrap()),
              u32::from_le_bytes(a[12..16].try_into().unwrap()),
            ]
          }
          // When bitmasks aren't specified, there's default RGB mask values. It
          // seems to be either 555 (16-bit) or 888 (32-bit).
          _ => match bits_per_pixel {
            16 => [0b11111 << 10, 0b11111 << 5, 0b11111, 0],
            32 => [0b11111111 << 16, 0b11111111 << 8, 0b11111111, 0],
            _ => [0, 0, 0, 0],
          },
        }
      }
      BmpInfoHeader::V2(BmpInfoHeaderV2 { red_mask, green_mask, blue_mask, .. }) => {
        [red_mask, green_mask, blue_mask, 0]
      }
      BmpInfoHeader::V3(BmpInfoHeaderV3 {
        red_mask, green_mask, blue_mask, alpha_mask, ..
      }) => [red_mask, green_mask, blue_mask, alpha_mask],
      BmpInfoHeader::V4(BmpInfoHeaderV4 {
        red_mask, green_mask, blue_mask, alpha_mask, ..
      }) => [red_mask, green_mask, blue_mask, alpha_mask],
      BmpInfoHeader::V5(BmpInfoHeaderV5 {
        red_mask, green_mask, blue_mask, alpha_mask, ..
      }) => [red_mask, green_mask, blue_mask, alpha_mask],
    };

    // we make our final storage once we know about the masks. if there is a
    // non-zero alpha mask then we assume the image is alpha aware and pixels
    // default to transparent black. otherwise we assume that the image doesn't
    // know about alpha and use opaque black as the default. This is significant
    // because the RLE compressions can skip touching some pixels entirely and
    // just leave the default color in place.
    let mut final_storage: Vec<P> = Vec::new();
    final_storage.try_reserve(pixel_count).map_err(|_| BmpError::AllocError)?;
    final_storage.resize(
      pixel_count,
      (if a_mask != 0 {
        r8g8b8a8_Unorm::default()
      } else {
        r8g8b8a8_Unorm { r: 0, g: 0, b: 0, a: 0xFF }
      })
      .into(),
    );

    let palette: Vec<r8g8b8a8_Unorm> = match info_header.palette_len() {
      0 => Vec::new(),
      count => {
        let mut v = Vec::new();
        v.try_reserve(count).map_err(|_| BmpError::AllocError)?;
        match info_header {
          BmpInfoHeader::Core(_) => {
            let bytes_needed = count * size_of::<[u8; 3]>();
            let (pal_slice, _) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            let pal_slice: &[[u8; 3]] = cast_slice(pal_slice);
            for [b, g, r] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Unorm { r, g, b, a: 0xFF });
            }
          }
          _ => {
            let bytes_needed = count * size_of::<[u8; 4]>();
            let (pal_slice, _) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            let pal_slice: &[[u8; 4]] = cast_slice(pal_slice);
            for [b, g, r, a] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Unorm { r, g, b, a });
            }
            if v.iter().copied().all(|c| c.a == 0) {
              v.iter_mut().for_each(|c| c.a = 0xFF);
            }
          }
        }
        v
      }
    };

    let pixel_data_start_index: usize = file_header.pixel_data_offset as usize;
    let pixel_data_end_index: usize = pixel_data_start_index + info_header.pixel_data_len();
    let pixel_data = if bytes.len() < pixel_data_end_index {
      return Err(BmpError::InsufficientBytes);
    } else {
      &bytes[pixel_data_start_index..pixel_data_end_index]
    };

    match compression {
      BmpCompression::RgbNoCompression
      | BmpCompression::Bitfields
      | BmpCompression::AlphaBitfields => {
        let bits_per_line: usize =
          bits_per_pixel.saturating_mul(info_header.width().unsigned_abs() as usize);
        let no_padding_bytes_per_line: usize =
          (bits_per_line / 8) + (((bits_per_line % 8) != 0) as usize);
        let bytes_per_line: usize =
          ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
        debug_assert!(no_padding_bytes_per_line <= bytes_per_line);
        debug_assert_eq!(bytes_per_line % 4, 0);
        if (pixel_data.len() % bytes_per_line) != 0
          || (pixel_data.len() / bytes_per_line) != (info_header.height().unsigned_abs() as usize)
        {
          return Err(BmpError::PixelDataIllegalLength);
        }
        //

        match bits_per_pixel {
          1 | 2 | 4 => {
            let (base_mask, base_down_shift) = match bits_per_pixel {
              1 => (0b1000_0000, 7),
              2 => (0b1100_0000, 6),
              4 => (0b1111_0000, 4),
              _ => unreachable!(),
            };
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                let mut x = 0;
                for byte in data_row.iter().copied() {
                  let mut mask: u8 = base_mask;
                  let mut down_shift: usize = base_down_shift;
                  while mask != 0 && x < width {
                    let pal_index = (byte & mask) >> down_shift;
                    let p: P = palette.get(pal_index as usize).copied().unwrap_or_default().into();
                    final_storage[y * width + x] = p;
                    //
                    mask >>= bits_per_pixel;
                    down_shift = down_shift.wrapping_sub(bits_per_pixel);
                    x += 1;
                  }
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
            }
          }
          8 => {
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                for (x, pal_index) in
                  data_row[..no_padding_bytes_per_line].iter().copied().enumerate()
                {
                  let p: P = palette.get(pal_index as usize).copied().unwrap_or_default().into();
                  final_storage[y * width + x] = p;
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
            }
          }
          24 => {
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                for (x, [b, g, r]) in
                  cast_slice::<u8, [u8; 3]>(&data_row[..no_padding_bytes_per_line])
                    .iter()
                    .copied()
                    .enumerate()
                {
                  let p: P = r8g8b8a8_Unorm { r, g, b, a: 0xFF }.into();
                  final_storage[y * width + x] = p;
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
            }
          }
          16 => {
            let r_shift: u32 = if r_mask != 0 { r_mask.trailing_zeros() } else { 0 };
            let g_shift: u32 = if g_mask != 0 { g_mask.trailing_zeros() } else { 0 };
            let b_shift: u32 = if b_mask != 0 { b_mask.trailing_zeros() } else { 0 };
            let a_shift: u32 = if a_mask != 0 { a_mask.trailing_zeros() } else { 0 };
            let r_max: f32 = (r_mask >> r_shift) as f32;
            let g_max: f32 = (g_mask >> g_shift) as f32;
            let b_max: f32 = (b_mask >> b_shift) as f32;
            let a_max: f32 = (a_mask >> a_shift) as f32;
            //
            #[rustfmt::skip]
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                for (x, data) in cast_slice::<u8, [u8; 2]>(&data_row[..no_padding_bytes_per_line])
                  .iter()
                  .copied()
                  .enumerate()
                {
                  // TODO: look at how SIMD this could be.
                  let u = u16::from_le_bytes(data) as u32;
                  let r: u8 = if r_mask != 0 { ((((u & r_mask) >> r_shift) as f32 / r_max) * 255.0) as u8 } else { 0 };
                  let g: u8 = if g_mask != 0 { ((((u & g_mask) >> g_shift) as f32 / g_max) * 255.0) as u8 } else { 0 };
                  let b: u8 = if b_mask != 0 { ((((u & b_mask) >> b_shift) as f32 / b_max) * 255.0) as u8 } else { 0 };
                  let a: u8 = if a_mask != 0 { ((((u & a_mask) >> a_shift) as f32 / a_max) * 255.0) as u8 } else { 0xFF };
                  let p: P = r8g8b8a8_Unorm { r, g, b, a }.into();
                  final_storage[y * width + x] = p;
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
            }
          }
          32 => {
            let r_shift: u32 = if r_mask != 0 { r_mask.trailing_zeros() } else { 0 };
            let g_shift: u32 = if g_mask != 0 { g_mask.trailing_zeros() } else { 0 };
            let b_shift: u32 = if b_mask != 0 { b_mask.trailing_zeros() } else { 0 };
            let a_shift: u32 = if a_mask != 0 { a_mask.trailing_zeros() } else { 0 };
            let r_max: f32 = (r_mask >> r_shift) as f32;
            let g_max: f32 = (g_mask >> g_shift) as f32;
            let b_max: f32 = (b_mask >> b_shift) as f32;
            let a_max: f32 = (a_mask >> a_shift) as f32;
            //
            #[rustfmt::skip]
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                for (x, data) in cast_slice::<u8, [u8; 4]>(&data_row[..no_padding_bytes_per_line])
                  .iter()
                  .copied()
                  .enumerate()
                {
                  // TODO: look at how SIMD this could be.
                  let u = u32::from_le_bytes(data);
                  let r: u8 = if r_mask != 0 { ((((u & r_mask) >> r_shift) as f32 / r_max) * 255.0) as u8 } else { 0 };
                  let g: u8 = if g_mask != 0 { ((((u & g_mask) >> g_shift) as f32 / g_max) * 255.0) as u8 } else { 0 };
                  let b: u8 = if b_mask != 0 { ((((u & b_mask) >> b_shift) as f32 / b_max) * 255.0) as u8 } else { 0 };
                  let a: u8 = if a_mask != 0 { ((((u & a_mask) >> a_shift) as f32 / a_max) * 255.0) as u8 } else { 0xFF };
                  let p: P = r8g8b8a8_Unorm { r, g, b, a }.into();
                  final_storage[y * width + x] = p;
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
            }
          }
          _ => return Err(BmpError::IllegalBitDepth),
        }
      }
      BmpCompression::RgbRLE8 => {
        // For the RLE encodings, there's either "encoded" pairs of bytes, or
        // "absolute" runs of bytes that are also always even in length (with a
        // padding byte if necessary). Thus, no matter what, the number of bytes
        // in the pixel data should always be even when we're processing RLE data.
        if pixel_data.len() % 2 != 0 {
          return Err(BmpError::PixelDataIllegalLength);
        }
        let mut it = cast_slice::<u8, [u8; 2]>(pixel_data).iter().copied();
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
        let mut x = 0;
        let mut y = height - 1;
        'iter_pull_rle8: while let Some([count, pal_index]) = it.next() {
          if count > 0 {
            // data run of the `pal_index` value's color.
            let p: P = palette.get(pal_index as usize).copied().unwrap_or_default().into();
            let count = count as usize;
            let i = y * width + x;
            let target_slice_mut: &mut [P] = if i + count <= final_storage.len() {
              &mut final_storage[i..(i + count)]
            } else {
              // this probably means the data was encoded wrong? We'll still fill
              // in some of the pixels at least, and if people can find a
              // counter-example we can fix this.
              &mut final_storage[i..]
            };
            target_slice_mut.iter_mut().for_each(|c| *c = p.clone());
            x += count;
          } else {
            match pal_index {
              0 => {
                // end of line.
                x = 0;
                y = y.saturating_sub(1);
              }
              1 => {
                // end of bitmap
                break 'iter_pull_rle8;
              }
              2 => {
                // position delta
                if let Some([d_right, d_up]) = it.next() {
                  x += d_right as usize;
                  y = y.saturating_sub(d_up as usize);
                } else {
                  return Err(BmpError::PixelDataIllegalRLEContent);
                }
              }
              mut raw_count => {
                while raw_count > 0 {
                  // process two bytes at a time, which we'll call `q` and `w` for
                  // lack of better names.
                  if let Some([q, w]) = it.next() {
                    // q byte
                    let p: P = palette.get(q as usize).copied().unwrap_or_default().into();
                    let i = y * width + x;
                    // If this goes OOB then that's the fault of the encoder
                    // that made this file and it's better to just drop some
                    // data than to panic.
                    if let Some(c) = final_storage.get_mut(i) {
                      *c = p;
                    }
                    x += 1;

                    // If `raw_count` is only 1 then we don't output the `w` byte.
                    if raw_count >= 2 {
                      // w byte
                      let p: P = palette.get(w as usize).copied().unwrap_or_default().into();
                      let i = y * width + x;
                      if let Some(c) = final_storage.get_mut(i) {
                        *c = p.clone();
                      }
                      x += 1;
                    }
                  } else {
                    return Err(BmpError::PixelDataIllegalRLEContent);
                  }
                  //
                  raw_count = raw_count.saturating_sub(2);
                }
              }
            }
          }
        }
      }
      BmpCompression::RgbRLE4 => {
        // RLE4 works *basically* how RLE8 does, except that every time we
        // process a byte as a color to output then it's actually two outputs
        // instead (upper bits then lower bits). The stuff about the escape
        // sequences and all that is still the same sort of thing.
        if pixel_data.len() % 2 != 0 {
          return Err(BmpError::PixelDataIllegalLength);
        }
        let mut it = cast_slice::<u8, [u8; 2]>(pixel_data).iter().copied();
        //
        let mut x = 0;
        let mut y = height - 1;
        //
        'iter_pull_rle4: while let Some([count, pal_index]) = it.next() {
          if count > 0 {
            // in this case, `count` is the number of indexes to output, and
            // `pal_index` is *two* pixel indexes (high bits then low bits). We'll
            // write the pair of them over and over in a loop however many times.
            if (pal_index >> 4) as usize >= palette.len() {
              // report error?
            }
            if (pal_index & 0b1111) as usize >= palette.len() {
              // report error?
            }
            let p_h: P = palette.get((pal_index >> 4) as usize).copied().unwrap_or_default().into();
            let p_l: P =
              palette.get((pal_index & 0b1111) as usize).copied().unwrap_or_default().into();
            let count = count as usize;
            debug_assert!(x < width, "x:{x}, width:{width}");
            debug_assert!(y < height, "y:{y}, height:{height}");
            let i = y * width + x;
            let target_slice_mut: &mut [P] = if i + count < final_storage.len() {
              &mut final_storage[i..(i + count)]
            } else {
              // this probably means the data was encoded wrong? We'll still fill
              // in some of the pixels at least, and if people can find a
              // counter-example we can fix this.
              &mut final_storage[i..]
            };
            let mut chunks_exact_mut = target_slice_mut.chunks_exact_mut(2);
            for chunk in chunks_exact_mut.by_ref() {
              chunk[0] = p_h.clone();
              chunk[1] = p_l.clone();
            }
            chunks_exact_mut.into_remainder().iter_mut().for_each(|c| {
              *c = p_h.clone();
            });
            x += count;
          } else {
            // If the count is zero then we use the same escape sequence scheme as
            // with the RLE8 format.
            match pal_index {
              0 => {
                // end of line.
                //print!("RLE4: == END OF LINE: before (x: {}, y: {})", x, y);
                x = 0;
                y = y.saturating_sub(1);
                //println!(" after (x: {}, y: {})", x, y);
              }
              1 => {
                // end of bitmap
                //println!("RLE4: ==>> END OF THE BITMAP");
                break 'iter_pull_rle4;
              }
              2 => {
                // delta
                if let Some([d_right, d_up]) = it.next() {
                  x += d_right as usize;
                  y = y.saturating_sub(d_up as usize);
                } else {
                  return Err(BmpError::PixelDataIllegalRLEContent);
                }
              }
              mut raw_count => {
                // in this case, we'll still have raw outputs for a sequence, but
                // `raw_count` is the number of indexes, and each 2 bytes in the
                // sequence is **four** output indexes. The only complication is
                // that we need to be sure we stop *as soon* as `raw_count` hits 0
                // because otherwise we'll mess up our `x` position (ruining all
                // future outputs on this scanline, and possibly ruining other
                // scanlines or something).
                while raw_count > 0 {
                  if let Some([q, w]) = it.next() {
                    for pal_index in [
                      ((q >> 4) as usize),
                      ((q & 0b1111) as usize),
                      ((w >> 4) as usize),
                      ((w & 0b1111) as usize),
                    ] {
                      let i = y * width + x;
                      if pal_index >= palette.len() {
                        // report error?
                      }
                      let p_h: P = palette.get(pal_index).copied().unwrap_or_default().into();
                      if let Some(c) = final_storage.get_mut(i) {
                        *c = p_h.clone();
                      }
                      x += 1;
                      raw_count = raw_count.saturating_sub(1);
                      if raw_count == 0 {
                        break;
                      }
                    }
                  } else {
                    return Err(BmpError::PixelDataIllegalRLEContent);
                  }
                }
              }
            }
          }
        }
      }
      // Note(Lokathor): Uh, I guess "the entire file is inside the 'pixel_array'
      // data" or whatever? We need example files that use this compression before
      // we can begin to check out what's going on here.
      BmpCompression::Jpeg => return Err(BmpError::ParserIncomplete),
      BmpCompression::Png => return Err(BmpError::ParserIncomplete),
      // Note(Lokathor): probably we never need to support this until someone asks?
      BmpCompression::CmykNoCompression => return Err(BmpError::ParserIncomplete),
      BmpCompression::CmykRLE4 => return Err(BmpError::ParserIncomplete),
      BmpCompression::CmykRLE8 => return Err(BmpError::ParserIncomplete),
    }

    let bitmap =
      crate::image::Bitmap { height: height as u32, width: width as u32, pixels: final_storage };
    Ok(bitmap)
  }
}

#[cfg(feature = "alloc")]
impl<P> crate::image::Palmap<u8, P>
where
  P: From<r8g8b8a8_Unorm> + Clone,
{
  /// Attempts to make a [Palmap](crate::image::Palmap) from BMP bytes.
  ///
  /// ## Failure
  /// Errors include, but are not limited to:
  /// * Incorrect Bmp File Header.
  /// * Allocation failure.
  #[cfg_attr(docs_rs, doc(cfg(all(feature = "bmp", feature = "miniz_oxide"))))]
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn try_from_bmp_bytes(bytes: &[u8]) -> Result<Self, BmpError> {
    use crate::image::Palmap;
    use alloc::vec::Vec;
    use bytemuck::cast_slice;
    use core::mem::size_of;
    //
    let (file_header, rest) = BmpFileHeader::try_from_bytes(bytes)?;
    if file_header.total_file_size as usize != bytes.len()
      || !(COMMON_BMP_TAGS.contains(&file_header.tag))
    {
      return Err(BmpError::ThisIsProbablyNotABmpFile);
    }
    let (info_header, mut rest) = BmpInfoHeader::try_from_bytes(rest)?;
    let compression = info_header.compression();
    let bits_per_pixel = info_header.bits_per_pixel() as usize;
    let width: u32 = info_header.width().unsigned_abs();
    let height: u32 = info_header.height().unsigned_abs();
    let pixel_count: usize = width.saturating_mul(height) as usize;

    #[allow(unused_assignments)]
    let palette: Vec<P> = match info_header.palette_len() {
      0 => Vec::new(),
      pal_count => {
        let mut v: Vec<r8g8b8a8_Unorm> = Vec::new();
        v.try_reserve(pal_count).map_err(|_| BmpError::AllocError)?;
        match info_header {
          BmpInfoHeader::Core(_) => {
            let bytes_needed = pal_count * size_of::<[u8; 3]>();
            let (pal_slice, new_rest) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            rest = new_rest;
            let pal_slice: &[[u8; 3]] = cast_slice(pal_slice);
            for [b, g, r] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Unorm { r, g, b, a: 0xFF });
            }
          }
          _ => {
            let bytes_needed = pal_count * size_of::<[u8; 4]>();
            let (pal_slice, new_rest) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            rest = new_rest;
            let pal_slice: &[[u8; 4]] = cast_slice(pal_slice);
            for [b, g, r, a] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Unorm { r, g, b, a });
            }
            if v.iter().copied().all(|c| c.a == 0) {
              v.iter_mut().for_each(|c| c.a = 0xFF);
            }
          }
        }
        // unfortunately we need to double-allocate if we want to keep the generics.
        let mut pal_final: Vec<P> = Vec::new();
        v.try_reserve(pal_count).map_err(|_| BmpError::AllocError)?;
        pal_final.extend(v.iter().map(|rgba| P::from(*rgba)));
        pal_final
      }
    };

    let indexes: Vec<u8> = {
      let mut v = Vec::new();
      v.try_reserve(pixel_count).map_err(|_| BmpError::AllocError)?;
      v.resize(pixel_count, 0_u8);
      v
    };

    let mut palmap: Palmap<u8, P> = Palmap { width, height, indexes, palette };

    let pixel_data_start_index: usize = file_header.pixel_data_offset as usize;
    let pixel_data_end_index: usize = pixel_data_start_index + info_header.pixel_data_len();
    let pixel_data = if bytes.len() < pixel_data_end_index {
      return Err(BmpError::InsufficientBytes);
    } else {
      &bytes[pixel_data_start_index..pixel_data_end_index]
    };

    match compression {
      BmpCompression::RgbNoCompression
      | BmpCompression::Bitfields
      | BmpCompression::AlphaBitfields => {
        let bits_per_line: usize =
          bits_per_pixel.saturating_mul(info_header.width().unsigned_abs() as usize);
        let no_padding_bytes_per_line: usize =
          (bits_per_line / 8) + (((bits_per_line % 8) != 0) as usize);
        let bytes_per_line: usize =
          ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
        debug_assert!(no_padding_bytes_per_line <= bytes_per_line);
        debug_assert_eq!(bytes_per_line % 4, 0);
        if (pixel_data.len() % bytes_per_line) != 0
          || (pixel_data.len() / bytes_per_line) != (info_header.height().unsigned_abs() as usize)
        {
          return Err(BmpError::PixelDataIllegalLength);
        }
        //

        match bits_per_pixel {
          1 | 2 | 4 => {
            let (base_mask, base_down_shift) = match bits_per_pixel {
              1 => (0b1000_0000, 7),
              2 => (0b1100_0000, 6),
              4 => (0b1111_0000, 4),
              _ => unreachable!(),
            };
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                let y = y as u32;
                let mut x = 0;
                for byte in data_row.iter().copied() {
                  let mut mask: u8 = base_mask;
                  let mut down_shift: usize = base_down_shift;
                  while mask != 0 && x < width {
                    let pal_index = (byte & mask) >> down_shift;
                    if let Some(i) = palmap.get_mut(x, y) {
                      *i = pal_index;
                    }
                    //
                    mask >>= bits_per_pixel;
                    down_shift = down_shift.wrapping_sub(bits_per_pixel);
                    x += 1;
                  }
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
            }
          }
          8 => {
            let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
              for (y, data_row) in i {
                for (x, pal_index) in
                  data_row[..no_padding_bytes_per_line].iter().copied().enumerate()
                {
                  if let Some(i) = palmap.get_mut(x as u32, y as u32) {
                    *i = pal_index;
                  }
                }
              }
            };
            if info_header.height() < 0 {
              per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
            } else {
              per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
            }
          }
          24 => {
            return Err(BmpError::NotAPalettedBmp);
          }
          16 => {
            return Err(BmpError::NotAPalettedBmp);
          }
          32 => {
            return Err(BmpError::NotAPalettedBmp);
          }
          _ => return Err(BmpError::IllegalBitDepth),
        }
      }
      BmpCompression::RgbRLE8 => {
        // For the RLE encodings, there's either "encoded" pairs of bytes, or
        // "absolute" runs of bytes that are also always even in length (with a
        // padding byte if necessary). Thus, no matter what, the number of bytes
        // in the pixel data should always be even when we're processing RLE data.
        if pixel_data.len() % 2 != 0 {
          return Err(BmpError::PixelDataIllegalLength);
        }
        let mut it = cast_slice::<u8, [u8; 2]>(pixel_data).iter().copied();
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
        let mut x: u32 = 0;
        let mut y: u32 = height - 1;
        'iter_pull_rle8: while let Some([count, pal_index]) = it.next() {
          if count > 0 {
            // data run of the `pal_index` value.
            for _ in 0..count {
              if let Some(i) = palmap.get_mut(x, y) {
                *i = pal_index;
              }
              x += 1;
            }
          } else {
            match pal_index {
              0 => {
                // end of line.
                x = 0;
                y = y.saturating_sub(1);
              }
              1 => {
                // end of bitmap
                break 'iter_pull_rle8;
              }
              2 => {
                // position delta
                if let Some([d_right, d_up]) = it.next() {
                  x += u32::from(d_right);
                  y = y.saturating_sub(u32::from(d_up));
                } else {
                  return Err(BmpError::PixelDataIllegalRLEContent);
                }
              }
              mut raw_count => {
                while raw_count > 0 {
                  // process two bytes at a time, which we'll call `q` and `w` for
                  // lack of better names.
                  if let Some([q, w]) = it.next() {
                    // q byte
                    if let Some(i) = palmap.get_mut(x, y) {
                      *i = q;
                    }
                    x += 1;

                    // If `raw_count` is only 1 then we don't output the `w` byte.
                    if raw_count >= 2 {
                      // w byte
                      if let Some(i) = palmap.get_mut(x, y) {
                        *i = w;
                      }
                      x += 1;
                    }
                  } else {
                    return Err(BmpError::PixelDataIllegalRLEContent);
                  }
                  //
                  raw_count = raw_count.saturating_sub(2);
                }
              }
            }
          }
        }
      }
      BmpCompression::RgbRLE4 => {
        // RLE4 works *basically* how RLE8 does, except that every time we
        // process a byte as a color to output then it's actually two outputs
        // instead (upper bits then lower bits). The stuff about the escape
        // sequences and all that is still the same sort of thing.
        if pixel_data.len() % 2 != 0 {
          return Err(BmpError::PixelDataIllegalLength);
        }
        let mut it = cast_slice::<u8, [u8; 2]>(pixel_data).iter().copied();
        //
        let mut x = 0;
        let mut y = height - 1;
        //
        'iter_pull_rle4: while let Some([count, pal_index]) = it.next() {
          if count > 0 {
            // in this case, `count` is the number of indexes to output, and
            // `pal_index` is *two* pixel indexes (high bits then low bits). We'll
            // write the pair of them over and over in a loop however many times.
            let h: u8 = pal_index >> 4;
            let l: u8 = pal_index & 0b1111;
            let count = count;
            for _ in 0..count {
              if let Some(i) = palmap.get_mut(x, y) {
                *i = h;
              }
              if let Some(i) = palmap.get_mut(x + 1, y) {
                *i = l;
              }
              x += 2;
            }
          } else {
            // If the count is zero then we use the same escape sequence scheme as
            // with the RLE8 format.
            match pal_index {
              0 => {
                // end of line.
                //print!("RLE4: == END OF LINE: before (x: {}, y: {})", x, y);
                x = 0;
                y = y.saturating_sub(1);
                //println!(" after (x: {}, y: {})", x, y);
              }
              1 => {
                // end of bitmap
                //println!("RLE4: ==>> END OF THE BITMAP");
                break 'iter_pull_rle4;
              }
              2 => {
                // delta
                if let Some([d_right, d_up]) = it.next() {
                  x += u32::from(d_right);
                  y = y.saturating_sub(u32::from(d_up));
                } else {
                  return Err(BmpError::PixelDataIllegalRLEContent);
                }
              }
              mut raw_count => {
                // in this case, we'll still have raw outputs for a sequence, but
                // `raw_count` is the number of indexes, and each 2 bytes in the
                // sequence is **four** output indexes. The only complication is
                // that we need to be sure we stop *as soon* as `raw_count` hits 0
                // because otherwise we'll mess up our `x` position (ruining all
                // future outputs on this scanline, and possibly ruining other
                // scanlines or something).
                while raw_count > 0 {
                  if let Some([q, w]) = it.next() {
                    for pal_index in [(q >> 4), (q & 0b1111), (w >> 4), (w & 0b1111)] {
                      if let Some(i) = palmap.get_mut(x, y) {
                        *i = pal_index;
                      }
                      x += 1;
                      raw_count = raw_count.saturating_sub(1);
                      if raw_count == 0 {
                        break;
                      }
                    }
                  } else {
                    return Err(BmpError::PixelDataIllegalRLEContent);
                  }
                }
              }
            }
          }
        }
      }
      // Note(Lokathor): Uh, I guess "the entire file is inside the 'pixel_array'
      // data" or whatever? We need example files that use this compression before
      // we can begin to check out what's going on here.
      BmpCompression::Jpeg => return Err(BmpError::ParserIncomplete),
      BmpCompression::Png => return Err(BmpError::ParserIncomplete),
      // Note(Lokathor): probably we never need to support this until someone asks?
      BmpCompression::CmykNoCompression => return Err(BmpError::ParserIncomplete),
      BmpCompression::CmykRLE4 => return Err(BmpError::ParserIncomplete),
      BmpCompression::CmykRLE8 => return Err(BmpError::ParserIncomplete),
    }

    Ok(palmap)
  }
}
