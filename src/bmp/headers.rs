#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use bytemuck::cast;

use super::*;
use crate::{parser_helpers::*, SrgbIntent};

pub const COMMON_BMP_TAGS: &[AsciiArray<2>] = &[
  AsciiArray(*b"BM"),
  AsciiArray(*b"BA"),
  AsciiArray(*b"CI"),
  AsciiArray(*b"CP"),
  AsciiArray(*b"IC"),
  AsciiArray(*b"PT"),
];

/// The header at the start of all BMP files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BmpFileHeader {
  /// This is expected to be one of the following
  ///
  /// * BM: win3.1 or later
  /// * BA: OS/2 bitmap array
  /// * CI: OS/2 color icon
  /// * CP: OS/2 color pointer
  /// * IC: OS/2 icon
  /// * PT: OS/2 pointer
  pub tag: AsciiArray<2>,

  /// The total size of the file.
  ///
  /// If this doesn't match the actual size of the file, there might be some
  /// sort of data loss or corruption.
  pub total_file_size: u32,

  /// The byte index within the file where the bitmap data starts.
  pub pixel_data_offset: u32,
}
impl From<[u8; 14]> for BmpFileHeader {
  #[inline]
  #[must_use]
  fn from(value: [u8; 14]) -> Self {
    Self {
      tag: AsciiArray(value[0..2].try_into().unwrap()),
      total_file_size: u32_le(&value[2..6]),
      // 4 bytes skipped
      pixel_data_offset: u32_le(&value[10..14]),
    }
  }
}
impl From<BmpFileHeader> for [u8; 14] {
  fn from(h: BmpFileHeader) -> Self {
    let mut a = [0; 14];
    a[0..2].copy_from_slice(h.tag.0.as_slice());
    a[2..6].copy_from_slice(h.total_file_size.to_le_bytes().as_slice());
    // 4 bytes are left blank
    a[10..14].copy_from_slice(h.pixel_data_offset.to_le_bytes().as_slice());
    a
  }
}
impl BmpFileHeader {
  #[inline]
  pub fn try_from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), BmpError> {
    let (a, rest) = try_split_off_byte_array::<14>(bytes).ok_or(BmpError::InsufficientBytes)?;
    Ok((Self::from(a), rest))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BmpInfoHeader {
  Core(BmpInfoHeaderCore),
  Os22x(BmpInfoHeaderOs22x),
  V1(BmpInfoHeaderV1),
  V2(BmpInfoHeaderV2),
  V3(BmpInfoHeaderV3),
  V4(BmpInfoHeaderV4),
  V5(BmpInfoHeaderV5),
}
impl BmpInfoHeader {
  #[inline]
  pub fn try_from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), BmpError> {
    if bytes.len() < 4 {
      return Err(BmpError::InsufficientBytes);
    }
    Ok(match u32_le(&bytes[0..4]) {
      12 => {
        let (a, rest) = try_split_off_byte_array::<12>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::Core(BmpInfoHeaderCore::try_from(a)?), rest)
      }
      16 => {
        let (a, rest) = try_split_off_byte_array::<16>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::Os22x(BmpInfoHeaderOs22x::try_from(a)?), rest)
      }
      64 => {
        let (a, rest) = try_split_off_byte_array::<64>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::Os22x(BmpInfoHeaderOs22x::try_from(a)?), rest)
      }
      40 => {
        let (a, rest) = try_split_off_byte_array::<40>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::V1(BmpInfoHeaderV1::try_from(a)?), rest)
      }
      52 => {
        let (a, rest) = try_split_off_byte_array::<52>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::V2(BmpInfoHeaderV2::try_from(a)?), rest)
      }
      56 => {
        let (a, rest) = try_split_off_byte_array::<56>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::V3(BmpInfoHeaderV3::try_from(a)?), rest)
      }
      108 => {
        let (a, rest) =
          try_split_off_byte_array::<108>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::V4(BmpInfoHeaderV4::try_from(a)?), rest)
      }
      124 => {
        let (a, rest) =
          try_split_off_byte_array::<124>(bytes).ok_or(BmpError::InsufficientBytes)?;
        (Self::V5(BmpInfoHeaderV5::try_from(a)?), rest)
      }
      _ => return Err(BmpError::UnknownHeaderLength),
    })
  }

  #[inline]
  #[must_use]
  pub const fn width(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { width, .. }) => width as i32,
      Self::Os22x(BmpInfoHeaderOs22x { width, .. }) => width,
      Self::V1(BmpInfoHeaderV1 { width, .. }) => width,
      Self::V2(BmpInfoHeaderV2 { width, .. }) => width,
      Self::V3(BmpInfoHeaderV3 { width, .. }) => width,
      Self::V4(BmpInfoHeaderV4 { width, .. }) => width,
      Self::V5(BmpInfoHeaderV5 { width, .. }) => width,
    }
  }
  #[inline]
  #[must_use]
  pub const fn height(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { height, .. }) => height as i32,
      Self::Os22x(BmpInfoHeaderOs22x { height, .. }) => height,
      Self::V1(BmpInfoHeaderV1 { height, .. }) => height,
      Self::V2(BmpInfoHeaderV2 { height, .. }) => height,
      Self::V3(BmpInfoHeaderV3 { height, .. }) => height,
      Self::V4(BmpInfoHeaderV4 { height, .. }) => height,
      Self::V5(BmpInfoHeaderV5 { height, .. }) => height,
    }
  }
  #[inline]
  #[must_use]
  pub const fn bits_per_pixel(self) -> u16 {
    match self {
      Self::Core(BmpInfoHeaderCore { bits_per_pixel, .. }) => bits_per_pixel,
      Self::Os22x(BmpInfoHeaderOs22x { bits_per_pixel, .. }) => bits_per_pixel,
      Self::V1(BmpInfoHeaderV1 { bits_per_pixel, .. }) => bits_per_pixel,
      Self::V2(BmpInfoHeaderV2 { bits_per_pixel, .. }) => bits_per_pixel,
      Self::V3(BmpInfoHeaderV3 { bits_per_pixel, .. }) => bits_per_pixel,
      Self::V4(BmpInfoHeaderV4 { bits_per_pixel, .. }) => bits_per_pixel,
      Self::V5(BmpInfoHeaderV5 { bits_per_pixel, .. }) => bits_per_pixel,
    }
  }
  #[inline]
  #[must_use]
  pub const fn compression(self) -> BmpCompression {
    match self {
      Self::Core(BmpInfoHeaderCore { .. }) => BmpCompression::RgbNoCompression,
      Self::Os22x(BmpInfoHeaderOs22x { compression, .. }) => compression,
      Self::V1(BmpInfoHeaderV1 { compression, .. }) => compression,
      Self::V2(BmpInfoHeaderV2 { compression, .. }) => compression,
      Self::V3(BmpInfoHeaderV3 { compression, .. }) => compression,
      Self::V4(BmpInfoHeaderV4 { compression, .. }) => compression,
      Self::V5(BmpInfoHeaderV5 { compression, .. }) => compression,
    }
  }
}

/// Header for Windows 2.0 and OS/2 1.x images.
///
/// Unlikely to be seen in modern times.
///
/// Corresponds to the the 12 byte `BITMAPCOREHEADER` struct (aka
/// `OS21XBITMAPHEADER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderCore {
  /// Width in pixels
  pub width: i16,

  /// Height in pixels.
  ///
  /// In later versions of BMP, negative height means that the image origin is
  /// the top left and rows go down. Otherwise the origin is the bottom left,
  /// and rows go up. In this early version values are expected to always be
  /// positive, but if we do see a negative height here then probably we want to
  /// follow the same origin-flipping convention.
  pub height: i16,

  /// In this version of BMP, all colors are expected to be indexed, and this is
  /// the bits per index value (8 or less). An appropriate palette value should
  /// also be present in the bitmap.
  pub bits_per_pixel: u16,
}
impl TryFrom<[u8; 12]> for BmpInfoHeaderCore {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 12]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i16_le(&a[4..6]);
    let height = i16_le(&a[6..8]);
    let _color_planes = u16_le(&a[8..10]);
    let bits_per_pixel = u16_le(&a[10..12]);
    if size != 12 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self { width, height, bits_per_pixel })
    }
  }
}
impl From<BmpInfoHeaderCore> for [u8; 12] {
  #[inline]
  #[must_use]
  fn from(h: BmpInfoHeaderCore) -> Self {
    let mut a = [0; 12];
    a[0..4].copy_from_slice(12_u32.to_le_bytes().as_slice());
    a[4..6].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[6..8].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[8..10].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[10..12].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a
  }
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

/// Header for Windows 3.1 or later.
///
/// This is the most commonly used header, unless the image actually needs to
/// take advantage of a more advanced feature.
///
/// Corresponds to the 40 byte `BITMAPINFOHEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderV1 {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,
}
impl TryFrom<[u8; 40]> for BmpInfoHeaderV1 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 40]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let _color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    if size != 40 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
      })
    }
  }
}
impl From<BmpInfoHeaderV1> for [u8; 40] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderV1) -> Self {
    let mut a = [0; 40];
    a[0..4].copy_from_slice(40_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Header for OS/2 2.x and later.
///
/// This extends the [BmpInfoHeaderV1] with some additional information.
///
/// Corresponds to the `OS22XBITMAPHEADER` (aka `BITMAPINFOHEADER2` in some
/// docs), which can be either 64 bytes, or 16 bytes. The 16 byte version works
/// like it's just the first part of the 64 byte version, with the rest as
/// implied zeroes. [TryFrom] impls are provided for both sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderOs22x {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,

  /// If this is any value other than `None` then the horizontal and vertical
  /// pixels per meter values are actually in some other measurement units.
  /// However, no other values were ever officially defined.
  pub resolution_units: Option<NonZeroU16>,

  /// If this is any value other than `None` then the origin is some place other
  /// than the lower left. However, no other values were ever defined, and the
  /// `height` field can already also be used specify an upper-left origin, so
  /// this field is kinda useless.
  pub pixel_origin: Option<NonZeroU16>,

  /// The halftoning algorithm of the image data.
  pub halftoning: Halftoning,

  /// A value other than `None` indicates that the color table entries are
  /// something other than RGB. However, no other values are officially defined.
  pub color_table_format: Option<NonZeroU32>,
}
impl TryFrom<[u8; 64]> for BmpInfoHeaderOs22x {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 64]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let _color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    let resolution_units = onz_u16_le(&a[40..42]);
    // 2 bytes padding
    let pixel_origin = onz_u16_le(&a[44..46]);
    let halftoning = Halftoning::from({
      let x: [u8; 10] = a[46..56].try_into().unwrap();
      x
    });
    let color_table_format = onz_u32_le(&a[56..60]);
    // 4 bytes padding
    if size != 64 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
        resolution_units,
        pixel_origin,
        halftoning,
        color_table_format,
      })
    }
  }
}
impl TryFrom<[u8; 16]> for BmpInfoHeaderOs22x {
  type Error = BmpError;
  /// The 16 byte version is just like the 64 byte version with all extra bytes
  /// effectively zeroed.
  #[inline]
  fn try_from(a: [u8; 16]) -> Result<Self, Self::Error> {
    use BmpError::*;
    if u32_le(&a[0..4]) != 16 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      // Note(Lokathor): To cut down on code duplication, we'll just create a dummy
      // header and pass it along to the "real" version (the 64 byte version).
      let mut full = [0; 64];
      full[0..4].copy_from_slice(64_u32.to_le_bytes().as_slice());
      full[4..16].copy_from_slice(&a[4..16]);
      Self::try_from(full)
    }
  }
}
impl From<BmpInfoHeaderOs22x> for [u8; 64] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderOs22x) -> Self {
    let mut a = [0; 64];
    a[0..4].copy_from_slice(64_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a[40..42].copy_from_slice(cast::<_,u16>(h.resolution_units).to_le_bytes().as_slice());
    // 2 bytes padding
    a[44..46].copy_from_slice(cast::<_,u16>(h.pixel_origin).to_le_bytes().as_slice());
    a[46..56].copy_from_slice(<[u8;10]>::from(h.halftoning).as_slice());
    a[56..60].copy_from_slice(cast::<_,u32>(h.color_table_format).to_le_bytes().as_slice());
    // 4 bytes padding
    a
  }
}

/// InfoHeader version 2.
///
/// This is mostly undocumented, so new files are unlikely to use it.
///
/// Compared to V1, it adds RGB bit masks.
///
/// Corresponds to the 52 byte `BITMAPV2INFOHEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderV2 {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,

  /// Bit mask of where the red bits are located.
  pub red_mask: u32,

  /// Bit mask of where the green bits are located.
  pub green_mask: u32,

  /// Bit mask of where the blue bits are located.
  pub blue_mask: u32,
}
impl TryFrom<[u8; 52]> for BmpInfoHeaderV2 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 52]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    let red_mask = u32_le(&a[40..44]);
    let green_mask = u32_le(&a[44..48]);
    let blue_mask = u32_le(&a[48..52]);
    if size != 52 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
        red_mask,
        green_mask,
        blue_mask,
      })
    }
  }
}
impl From<BmpInfoHeaderV2> for [u8; 52] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderV2) -> Self {
    let mut a = [0; 52];
    a[0..4].copy_from_slice(52_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a[40..44].copy_from_slice(h.red_mask.to_le_bytes().as_slice());
    a[44..48].copy_from_slice(h.green_mask.to_le_bytes().as_slice());
    a[48..52].copy_from_slice(h.blue_mask.to_le_bytes().as_slice());
    a
  }
}

/// InfoHeader version 3.
///
/// This is mostly undocumented, so new files are unlikely to use it.
///
/// Compared to V2, it adds an alpha bit masks.
///
/// Corresponds to the 56 byte `BITMAPV3INFOHEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderV3 {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,

  /// Bit mask of where the red bits are located.
  pub red_mask: u32,

  /// Bit mask of where the green bits are located.
  pub green_mask: u32,

  /// Bit mask of where the blue bits are located.
  pub blue_mask: u32,

  /// Bit mask of where the alpha bits are located.
  pub alpha_mask: u32,
}
impl TryFrom<[u8; 56]> for BmpInfoHeaderV3 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 56]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let _color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    let red_mask = u32_le(&a[40..44]);
    let green_mask = u32_le(&a[44..48]);
    let blue_mask = u32_le(&a[48..52]);
    let alpha_mask = u32_le(&a[52..56]);
    if size != 56 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
        red_mask,
        green_mask,
        blue_mask,
        alpha_mask,
      })
    }
  }
}
impl From<BmpInfoHeaderV3> for [u8; 56] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderV3) -> Self {
    let mut a = [0; 56];
    a[0..4].copy_from_slice(56_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a[40..44].copy_from_slice(h.red_mask.to_le_bytes().as_slice());
    a[44..48].copy_from_slice(h.green_mask.to_le_bytes().as_slice());
    a[48..52].copy_from_slice(h.blue_mask.to_le_bytes().as_slice());
    a[52..56].copy_from_slice(h.alpha_mask.to_le_bytes().as_slice());
    a
  }
}

const LCS_CALIBRATED_RGB: u32 = 0x00000000;
const LCS_sRGB: u32 = 0x7352_4742;
/// This is b"Win " in little-endian, I'm not kidding.
const LCS_WINDOWS_COLOR_SPACE: u32 = 0x5769_6E20;
const PROFILE_LINKED: u32 = 0x4C49_4E4B;
const PROFILE_EMBEDDED: u32 = 0x4D42_4544;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BmpColorspace {
  /// The usual sRGB colorspace.
  Srgb,

  /// The windows default color space (On windows 10, this is also sRGB).
  WindowsDefault,

  /// A profile elsewhere is linked to (by name).
  LinkedProfile,

  /// A profile is embedded into the bitmap itself.
  EmbeddedProfile,

  /// The colorspace is calibrated according to the info given.
  Calibrated { endpoints: CIEXYZTRIPLE, gamma_red: u32, gamma_green: u32, gamma_blue: u32 },

  /// The colorspace tag was unknown.
  ///
  /// In this case, the endpoints and gamma values are still kept for you, but
  /// the data might be nonsensical values (including possibly just zeroed).
  Unknown { endpoints: CIEXYZTRIPLE, gamma_red: u32, gamma_green: u32, gamma_blue: u32 },
}
impl From<[u8; 52]> for BmpColorspace {
  fn from(a: [u8; 52]) -> Self {
    match u32_le(&a[0..4]) {
      LCS_CALIBRATED_RGB => BmpColorspace::Calibrated {
        endpoints: CIEXYZTRIPLE {
          red: CIEXYZ { x: u32_le(&a[4..8]), y: u32_le(&a[8..12]), z: u32_le(&a[12..16]) },
          green: CIEXYZ { x: u32_le(&a[16..20]), y: u32_le(&a[20..24]), z: u32_le(&a[24..28]) },
          blue: CIEXYZ { x: u32_le(&a[28..32]), y: u32_le(&a[32..36]), z: u32_le(&a[36..40]) },
        },
        gamma_red: u32_le(&a[40..44]),
        gamma_green: u32_le(&a[44..48]),
        gamma_blue: u32_le(&a[48..52]),
      },
      LCS_sRGB => BmpColorspace::Srgb,
      LCS_WINDOWS_COLOR_SPACE => BmpColorspace::WindowsDefault,
      PROFILE_LINKED => BmpColorspace::LinkedProfile,
      PROFILE_EMBEDDED => BmpColorspace::EmbeddedProfile,
      _ => BmpColorspace::Unknown {
        endpoints: CIEXYZTRIPLE {
          red: CIEXYZ { x: u32_le(&a[4..8]), y: u32_le(&a[8..12]), z: u32_le(&a[12..16]) },
          green: CIEXYZ { x: u32_le(&a[16..20]), y: u32_le(&a[20..24]), z: u32_le(&a[24..28]) },
          blue: CIEXYZ { x: u32_le(&a[28..32]), y: u32_le(&a[32..36]), z: u32_le(&a[36..40]) },
        },
        gamma_red: u32_le(&a[40..44]),
        gamma_green: u32_le(&a[44..48]),
        gamma_blue: u32_le(&a[48..52]),
      },
    }
  }
}
impl From<BmpColorspace> for [u8; 52] {
  fn from(c: BmpColorspace) -> Self {
    let mut a = [0; 52];
    match c {
      BmpColorspace::Srgb => {
        a[0..4].copy_from_slice(LCS_sRGB.to_le_bytes().as_slice());
      }
      BmpColorspace::WindowsDefault => {
        a[0..4].copy_from_slice(LCS_WINDOWS_COLOR_SPACE.to_le_bytes().as_slice());
      }
      BmpColorspace::LinkedProfile => {
        a[0..4].copy_from_slice(PROFILE_LINKED.to_le_bytes().as_slice());
      }
      BmpColorspace::EmbeddedProfile => {
        a[0..4].copy_from_slice(PROFILE_EMBEDDED.to_le_bytes().as_slice());
      }
      BmpColorspace::Calibrated { endpoints, gamma_red, gamma_green, gamma_blue } => {
        a[0..4].copy_from_slice(LCS_CALIBRATED_RGB.to_le_bytes().as_slice());
        a[4..8].copy_from_slice(endpoints.red.x.to_le_bytes().as_slice());
        a[8..12].copy_from_slice(endpoints.red.y.to_le_bytes().as_slice());
        a[12..16].copy_from_slice(endpoints.red.z.to_le_bytes().as_slice());
        a[16..20].copy_from_slice(endpoints.green.x.to_le_bytes().as_slice());
        a[20..24].copy_from_slice(endpoints.green.y.to_le_bytes().as_slice());
        a[24..28].copy_from_slice(endpoints.green.z.to_le_bytes().as_slice());
        a[28..32].copy_from_slice(endpoints.blue.x.to_le_bytes().as_slice());
        a[32..36].copy_from_slice(endpoints.blue.y.to_le_bytes().as_slice());
        a[36..40].copy_from_slice(endpoints.blue.z.to_le_bytes().as_slice());
        a[40..44].copy_from_slice(gamma_red.to_le_bytes().as_slice());
        a[44..48].copy_from_slice(gamma_green.to_le_bytes().as_slice());
        a[48..52].copy_from_slice(gamma_blue.to_le_bytes().as_slice());
      }
      BmpColorspace::Unknown { endpoints, gamma_red, gamma_green, gamma_blue } => {
        // this is a made up value for unknown color spaces, it's hopefully not
        // gonna clash with anything else.
        a[0..4].copy_from_slice((u32::MAX - 1).to_le_bytes().as_slice());
        a[4..8].copy_from_slice(endpoints.red.x.to_le_bytes().as_slice());
        a[8..12].copy_from_slice(endpoints.red.y.to_le_bytes().as_slice());
        a[12..16].copy_from_slice(endpoints.red.z.to_le_bytes().as_slice());
        a[16..20].copy_from_slice(endpoints.green.x.to_le_bytes().as_slice());
        a[20..24].copy_from_slice(endpoints.green.y.to_le_bytes().as_slice());
        a[24..28].copy_from_slice(endpoints.green.z.to_le_bytes().as_slice());
        a[28..32].copy_from_slice(endpoints.blue.x.to_le_bytes().as_slice());
        a[32..36].copy_from_slice(endpoints.blue.y.to_le_bytes().as_slice());
        a[36..40].copy_from_slice(endpoints.blue.z.to_le_bytes().as_slice());
        a[40..44].copy_from_slice(gamma_red.to_le_bytes().as_slice());
        a[44..48].copy_from_slice(gamma_green.to_le_bytes().as_slice());
        a[48..52].copy_from_slice(gamma_blue.to_le_bytes().as_slice());
      }
    }
    a
  }
}

/// Fixed point, 2.30
pub type FXPT2DOT30 = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct CIEXYZ {
  pub x: FXPT2DOT30,
  pub y: FXPT2DOT30,
  pub z: FXPT2DOT30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct CIEXYZTRIPLE {
  pub red: CIEXYZ,
  pub green: CIEXYZ,
  pub blue: CIEXYZ,
}

/// InfoHeader version 4.
///
/// Compared to V3, it adds color space info.
///
/// Corresponds to the 108 byte `BITMAPV4HEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderV4 {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  ///
  /// The value 0 is also allowed, which indicates that a Jpeg or Png file is
  /// contained in this bitmap, which will have the bits per pixel info.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,

  /// Bit mask of where the red bits are located.
  pub red_mask: u32,

  /// Bit mask of where the green bits are located.
  pub green_mask: u32,

  /// Bit mask of where the blue bits are located.
  pub blue_mask: u32,

  /// Bit mask of where the alpha bits are located.
  pub alpha_mask: u32,

  /// Colorspace info for the bitmap.
  ///
  /// For a V4 header, this should always be [BmpColorspace::Calibrated].
  pub colorspace: BmpColorspace,
}
impl TryFrom<[u8; 108]> for BmpInfoHeaderV4 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 108]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let _color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    let red_mask = u32_le(&a[40..44]);
    let green_mask = u32_le(&a[44..48]);
    let blue_mask = u32_le(&a[48..52]);
    let alpha_mask = u32_le(&a[52..56]);
    let colorspace = BmpColorspace::from({
      let x: [u8; 52] = a[56..108].try_into().unwrap();
      x
    });
    if size != 108 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
        red_mask,
        green_mask,
        blue_mask,
        alpha_mask,
        colorspace,
      })
    }
  }
}
impl From<BmpInfoHeaderV4> for [u8; 108] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderV4) -> Self {
    let mut a = [0; 108];
    a[0..4].copy_from_slice(40_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a[40..44].copy_from_slice(h.red_mask.to_le_bytes().as_slice());
    a[44..48].copy_from_slice(h.green_mask.to_le_bytes().as_slice());
    a[48..52].copy_from_slice(h.blue_mask.to_le_bytes().as_slice());
    a[52..56].copy_from_slice(h.alpha_mask.to_le_bytes().as_slice());
    a[56..108].copy_from_slice(<[u8;52]>::from(h.colorspace).as_slice());
    a
  }
}

/// InfoHeader version 5.
///
/// Compared to V5, it adds more color profile information.
///
/// Corresponds to the 124 byte `BITMAPV5HEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmpInfoHeaderV5 {
  /// Image pixel width
  pub width: i32,

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  pub height: i32,

  /// Should be 1, 4, 8, 16, 24, or 32.
  ///
  /// The value 0 is also allowed, which indicates that a Jpeg or Png file is
  /// contained in this bitmap, which will have the bits per pixel info.
  pub bits_per_pixel: u16,

  /// The compression style of the image data.
  pub compression: BmpCompression,

  /// The number of bytes in the raw bitmap data.
  ///
  /// If the image compression is [BmpCompression::RgbNoCompression] then `None`
  /// can be used.
  pub image_byte_size: Option<NonZeroU32>,

  /// horizontal pixels per meter
  pub h_ppm: i32,

  /// vertical pixels per meter
  pub v_ppm: i32,

  /// Palette length.
  ///
  /// A value of `None` indicates that the full `2**N` palette is used (where
  /// `N` is the image bit depth).
  pub palette_len: Option<NonZeroU32>,

  /// The number of "important" colors.
  ///
  /// A value of `None` indicates that all colors are important.
  ///
  /// This field is generally ignored.
  pub important_colors: Option<NonZeroU32>,

  /// Bit mask of where the red bits are located.
  pub red_mask: u32,

  /// Bit mask of where the green bits are located.
  pub green_mask: u32,

  /// Bit mask of where the blue bits are located.
  pub blue_mask: u32,

  /// Bit mask of where the alpha bits are located.
  pub alpha_mask: u32,

  /// Colorspace info for the bitmap.
  ///
  /// For a V4 header, this should always be [BmpColorspace::Calibrated].
  pub colorspace: BmpColorspace,

  /// The sRGB intent of the image.
  ///
  /// A `None` value indicates that the intent was an invalid value when
  /// parsing. If this is `None`, writing the header into bytes will use a
  /// (semver exempt) unspecified default value.
  ///
  /// This isn't really supposed to be optional, but doing it like this allows
  /// the caller to keep the rest of the header data (or not) after a header is
  /// parsed. Otherwise, a bad intent value would force the entire header parse
  /// to fail (which is usually overkill).
  pub srgb_intent: Option<SrgbIntent>,

  /// The offset, in bytes, from the beginning of the `BITMAPV5HEADER` structure
  /// to the start of the profile data.
  ///
  /// * If the `colorspace` is [BmpColorspace::EmbeddedProfile] this is the
  ///   direct color profile data. Use the `embedded_profile_size` field to
  ///   determine the size.
  /// * If the `colorspace` is [BmpColorspace::LinkedProfile] this is the name
  ///   of the linked color profile. The name is a null-terminated string, and
  ///   it's using the Windows CodePage-1252 character set.
  /// * Otherwise ???
  pub profile_data: u32,

  /// The size, in bytes, of the embedded profile data.
  ///
  /// Otherwise this is... probably zero?
  pub embedded_profile_size: u32,
}
const LCS_GM_ABS_COLORIMETRIC: u32 = 0x00000008;
const LCS_GM_BUSINESS: u32 = 0x00000001;
const LCS_GM_GRAPHICS: u32 = 0x00000002;
const LCS_GM_IMAGES: u32 = 0x00000004;
impl TryFrom<[u8; 124]> for BmpInfoHeaderV5 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 124]) -> Result<Self, Self::Error> {
    use BmpError::*;
    let size = u32_le(&a[0..4]);
    let width = i32_le(&a[4..8]);
    let height = i32_le(&a[8..12]);
    let color_planes = u16_le(&a[12..14]);
    let bits_per_pixel = u16_le(&a[14..16]);
    let compression = BmpCompression::try_from(u32_le(&a[16..20]))?;
    let image_byte_size = onz_u32_le(&a[20..24]);
    let h_ppm = i32_le(&a[24..28]);
    let v_ppm = i32_le(&a[28..32]);
    let palette_len = onz_u32_le(&a[32..36]);
    let important_colors = onz_u32_le(&a[36..40]);
    let red_mask = u32_le(&a[40..44]);
    let green_mask = u32_le(&a[44..48]);
    let blue_mask = u32_le(&a[48..52]);
    let alpha_mask = u32_le(&a[52..56]);
    let colorspace = BmpColorspace::from({
      let x: [u8; 52] = a[56..108].try_into().unwrap();
      x
    });
    let srgb_intent = match u32_le(&a[108..112]) {
      LCS_GM_ABS_COLORIMETRIC => Some(SrgbIntent::AbsoluteColorimetric),
      LCS_GM_BUSINESS => Some(SrgbIntent::Saturation),
      LCS_GM_GRAPHICS => Some(SrgbIntent::RelativeColorimetric),
      LCS_GM_IMAGES => Some(SrgbIntent::Perceptual),
      _ => None,
    };
    let profile_data = u32_le(&a[112..116]);
    let embedded_profile_size = u32_le(&a[116..120]);
    // 4 bytes of padding
    if size != 124 {
      return Err(IncorrectSizeForThisInfoHeaderVersion);
    } else {
      Ok(Self {
        width,
        height,
        bits_per_pixel,
        compression,
        image_byte_size,
        h_ppm,
        v_ppm,
        palette_len,
        important_colors,
        red_mask,
        green_mask,
        blue_mask,
        alpha_mask,
        colorspace,
        srgb_intent,
        profile_data,
        embedded_profile_size,
      })
    }
  }
}
impl From<BmpInfoHeaderV5> for [u8; 124] {
  #[inline]
  #[must_use]
  #[rustfmt::skip]
  fn from(h: BmpInfoHeaderV5) -> Self {
    let mut a = [0; 124];
    a[0..4].copy_from_slice(40_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(u32::from(h.compression).to_le_bytes().as_slice());
    a[20..24].copy_from_slice(cast::<_,u32>(h.image_byte_size).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.h_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.v_ppm.to_le_bytes().as_slice());
    a[32..36].copy_from_slice(cast::<_,u32>(h.palette_len).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(cast::<_,u32>(h.important_colors).to_le_bytes().as_slice());
    a[40..44].copy_from_slice(h.red_mask.to_le_bytes().as_slice());
    a[44..48].copy_from_slice(h.green_mask.to_le_bytes().as_slice());
    a[48..52].copy_from_slice(h.blue_mask.to_le_bytes().as_slice());
    a[52..56].copy_from_slice(h.alpha_mask.to_le_bytes().as_slice());
    a[56..108].copy_from_slice(<[u8;52]>::from(h.colorspace).as_slice());
    a[108..112].copy_from_slice((match h.srgb_intent {
      Some(SrgbIntent::AbsoluteColorimetric) => LCS_GM_ABS_COLORIMETRIC,
      Some(SrgbIntent::Perceptual) => LCS_GM_IMAGES,
      Some(SrgbIntent::RelativeColorimetric) => LCS_GM_GRAPHICS,
      Some(SrgbIntent::Saturation) => LCS_GM_BUSINESS,
      None => LCS_GM_ABS_COLORIMETRIC,
    }).to_le_bytes().as_slice());
    a[112..116].copy_from_slice(h.profile_data.to_le_bytes().as_slice());
    a[116..120].copy_from_slice(h.embedded_profile_size.to_le_bytes().as_slice());
    // 4 bytes left blank
    a
  }
}
