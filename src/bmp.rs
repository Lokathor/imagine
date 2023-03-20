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

use crate::{
  image::{Bitmap, Palmap},
  sRGBIntent, AsciiArray,
};
use bytemuck::cast;
use core::{
  fmt::Write,
  num::{NonZeroU16, NonZeroU32},
};
use pixel_formats::r8g8b8a8_Srgb;

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
  /// The BMP file might be valid, but either way this library doesn't currently
  /// know how to parse it.
  ParserIncomplete,
}

#[inline]
#[must_use]
fn u16_le(bytes: &[u8]) -> u16 {
  u16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
fn i16_le(bytes: &[u8]) -> i16 {
  i16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
fn u32_le(bytes: &[u8]) -> u32 {
  u32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
fn i32_le(bytes: &[u8]) -> i32 {
  i32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
fn onz_u16_le(bytes: &[u8]) -> Option<NonZeroU16> {
  NonZeroU16::new(u16_le(bytes))
}

#[inline]
#[must_use]
fn onz_u32_le(bytes: &[u8]) -> Option<NonZeroU32> {
  NonZeroU32::new(u32_le(bytes))
}

fn try_split_off_byte_array<const N: usize>(bytes: &[u8]) -> Option<([u8; N], &[u8])> {
  if bytes.len() >= N {
    let (head, tail) = bytes.split_at(N);
    let a: [u8; N] = head.try_into().unwrap();
    Some((a, tail))
  } else {
    None
  }
}

/// Two-letter file tags commonly found at the start of a BMP file.
pub const COMMON_BMP_TAGS: &[AsciiArray<2>] = &[
  AsciiArray(*b"BM"),
  AsciiArray(*b"BA"),
  AsciiArray(*b"CI"),
  AsciiArray(*b"CP"),
  AsciiArray(*b"IC"),
  AsciiArray(*b"PT"),
];

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
  #[inline]
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
  /// Tries to get the file header and remaining bytes from the bytes of a BMP
  /// file.
  ///
  /// The bytes from here should be used to get the [BmpInfoHeader].
  #[inline]
  pub fn try_from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), BmpError> {
    let (a, rest) = try_split_off_byte_array::<14>(bytes).ok_or(BmpError::InsufficientBytes)?;
    Ok((Self::from(a), rest))
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

/// Colorspace data for the BMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
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
  #[inline]
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
  #[inline]
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
#[allow(missing_docs)]
pub struct CIEXYZ {
  pub x: FXPT2DOT30,
  pub y: FXPT2DOT30,
  pub z: FXPT2DOT30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[allow(missing_docs)]
pub struct CIEXYZTRIPLE {
  pub red: CIEXYZ,
  pub green: CIEXYZ,
  pub blue: CIEXYZ,
}

/// An enum over the various BMP info header versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[allow(missing_docs)]
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
  /// Tries to get the info header and remaining bytes.
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

  /// Image pixel width.
  #[inline]
  #[must_use]
  pub const fn width(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { width, .. }) => width as i32,
      Self::Os22x(BmpInfoHeaderOs22x { width, .. })
      | Self::V1(BmpInfoHeaderV1 { width, .. })
      | Self::V2(BmpInfoHeaderV2 { width, .. })
      | Self::V3(BmpInfoHeaderV3 { width, .. })
      | Self::V4(BmpInfoHeaderV4 { width, .. })
      | Self::V5(BmpInfoHeaderV5 { width, .. }) => width,
    }
  }

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  #[inline]
  #[must_use]
  pub const fn height(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { height, .. }) => height as i32,
      Self::Os22x(BmpInfoHeaderOs22x { height, .. })
      | Self::V1(BmpInfoHeaderV1 { height, .. })
      | Self::V2(BmpInfoHeaderV2 { height, .. })
      | Self::V3(BmpInfoHeaderV3 { height, .. })
      | Self::V4(BmpInfoHeaderV4 { height, .. })
      | Self::V5(BmpInfoHeaderV5 { height, .. }) => height,
    }
  }

  /// Bits per pixel, should be in the 1 to 32 range.
  #[inline]
  #[must_use]
  pub const fn bits_per_pixel(self) -> u16 {
    match self {
      Self::Core(BmpInfoHeaderCore { bits_per_pixel, .. })
      | Self::Os22x(BmpInfoHeaderOs22x { bits_per_pixel, .. })
      | Self::V1(BmpInfoHeaderV1 { bits_per_pixel, .. })
      | Self::V2(BmpInfoHeaderV2 { bits_per_pixel, .. })
      | Self::V3(BmpInfoHeaderV3 { bits_per_pixel, .. })
      | Self::V4(BmpInfoHeaderV4 { bits_per_pixel, .. })
      | Self::V5(BmpInfoHeaderV5 { bits_per_pixel, .. }) => bits_per_pixel,
    }
  }

  /// Compression method.
  #[inline]
  #[must_use]
  pub const fn compression(self) -> BmpCompression {
    match self {
      Self::Core(BmpInfoHeaderCore { .. }) => BmpCompression::RgbNoCompression,
      Self::Os22x(BmpInfoHeaderOs22x { compression, .. })
      | Self::V1(BmpInfoHeaderV1 { compression, .. })
      | Self::V2(BmpInfoHeaderV2 { compression, .. })
      | Self::V3(BmpInfoHeaderV3 { compression, .. })
      | Self::V4(BmpInfoHeaderV4 { compression, .. })
      | Self::V5(BmpInfoHeaderV5 { compression, .. }) => compression,
    }
  }

  /// Gets the number of palette entries.
  ///
  /// Meaning of a `None` value for the `palette_len` field changes depending on
  /// the bit depth of the image, so this method handles that difference for you
  /// and just gives you a single `u32` that's the *actual* number of entries on
  /// the palette.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self {
      Self::Core(BmpInfoHeaderCore { bits_per_pixel, .. }) => 1 << bits_per_pixel,
      Self::Os22x(x) => x.palette_len(),
      Self::V1(x) => x.palette_len(),
      Self::V2(x) => x.palette_len(),
      Self::V3(x) => x.palette_len(),
      Self::V4(x) => x.palette_len(),
      Self::V5(x) => x.palette_len(),
    }
  }

  /// Gets the number of bytes in the pixel data region of the file.
  #[inline]
  #[must_use]
  pub const fn pixel_data_len(self) -> usize {
    match self {
      Self::Core(BmpInfoHeaderCore { .. }) => {
        self.width().unsigned_abs().saturating_mul(self.height().unsigned_abs()) as usize
      }
      Self::Os22x(BmpInfoHeaderOs22x { image_byte_size, .. })
      | Self::V1(BmpInfoHeaderV1 { image_byte_size, .. })
      | Self::V2(BmpInfoHeaderV2 { image_byte_size, .. })
      | Self::V3(BmpInfoHeaderV3 { image_byte_size, .. })
      | Self::V4(BmpInfoHeaderV4 { image_byte_size, .. })
      | Self::V5(BmpInfoHeaderV5 { image_byte_size, .. }) => match image_byte_size {
        Some(x) => x.get() as usize,
        None => {
          let width_u = self.width().unsigned_abs() as usize;
          let height_u = self.height().unsigned_abs() as usize;
          let bits_per_line = width_u.saturating_mul(self.bits_per_pixel() as usize);
          let bytes_per_line_no_padding =
            (bits_per_line / 8) + (((bits_per_line % 8) != 0) as usize);
          let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
            + (((bytes_per_line_no_padding % 4) != 0) as usize))
            .saturating_mul(4);
          height_u.saturating_mul(bytes_per_line_padded)
        }
      },
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
impl BmpInfoHeaderV1 {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
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
  /// If the bit depth requires a palette and it's `None` that indicates that
  /// the full `2**N` palette is used (where `N` is the image bit depth).
  /// Otherwise, `None` indicates no palette.
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
impl BmpInfoHeaderOs22x {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
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
    if size != 52 {
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
impl BmpInfoHeaderV2 {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
impl BmpInfoHeaderV3 {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
  }
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
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
impl BmpInfoHeaderV4 {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
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
  pub srgb_intent: Option<sRGBIntent>,

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
impl TryFrom<[u8; 124]> for BmpInfoHeaderV5 {
  type Error = BmpError;
  #[inline]
  fn try_from(a: [u8; 124]) -> Result<Self, Self::Error> {
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
    let srgb_intent = match u32_le(&a[108..112]) {
      LCS_GM_ABS_COLORIMETRIC => Some(sRGBIntent::AbsoluteColorimetric),
      LCS_GM_BUSINESS => Some(sRGBIntent::Saturation),
      LCS_GM_GRAPHICS => Some(sRGBIntent::RelativeColorimetric),
      LCS_GM_IMAGES => Some(sRGBIntent::Perceptual),
      _ => None,
    };
    let profile_data = u32_le(&a[112..116]);
    let embedded_profile_size = u32_le(&a[116..120]);
    // 4 bytes of padding
    if size != 124 {
      Err(IncorrectSizeForThisInfoHeaderVersion)
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
      Some(sRGBIntent::AbsoluteColorimetric) => LCS_GM_ABS_COLORIMETRIC,
      Some(sRGBIntent::Perceptual) => LCS_GM_IMAGES,
      Some(sRGBIntent::RelativeColorimetric) => LCS_GM_GRAPHICS,
      Some(sRGBIntent::Saturation) => LCS_GM_BUSINESS,
      None => LCS_GM_ABS_COLORIMETRIC,
    }).to_le_bytes().as_slice());
    a[112..116].copy_from_slice(h.profile_data.to_le_bytes().as_slice());
    a[116..120].copy_from_slice(h.embedded_profile_size.to_le_bytes().as_slice());
    // 4 bytes left blank
    a
  }
}
impl BmpInfoHeaderV5 {
  /// Length of the palette.
  ///
  /// This method exists because if the listed value is zero then the palette
  /// length is implied, and this does the implied computation for you.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self.palette_len {
      Some(nzu32) => nzu32.get() as usize,
      None => {
        if self.bits_per_pixel <= 8 {
          1 << self.bits_per_pixel
        } else {
          0
        }
      }
    }
  }
}

#[cfg(feature = "alloc")]
impl<P> crate::image::Bitmap<P>
where
  P: From<r8g8b8a8_Srgb> + Clone,
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

    let [r_mask, g_mask, b_mask, a_mask] = match compression {
      BmpCompression::Bitfields => {
        let (a, new_rest) = try_split_off_byte_array::<{ size_of::<u32>() * 3 }>(rest)
          .ok_or(BmpError::InsufficientBytes)?;
        rest = new_rest;
        [
          u32::from_le_bytes(a[0..4].try_into().unwrap()),
          u32::from_le_bytes(a[4..8].try_into().unwrap()),
          u32::from_le_bytes(a[8..12].try_into().unwrap()),
          0,
        ]
      }
      BmpCompression::AlphaBitfields => {
        let (a, new_rest) = try_split_off_byte_array::<{ size_of::<u32>() * 4 }>(rest)
          .ok_or(BmpError::InsufficientBytes)?;
        rest = new_rest;
        [
          u32::from_le_bytes(a[0..4].try_into().unwrap()),
          u32::from_le_bytes(a[4..8].try_into().unwrap()),
          u32::from_le_bytes(a[8..12].try_into().unwrap()),
          u32::from_le_bytes(a[12..16].try_into().unwrap()),
        ]
      }
      // When bitmasks aren't specified, there's default RGB mask values based on
      // the bit depth, either 555 (16-bit) or 888 (32-bit).
      _ => match bits_per_pixel {
        16 => [0b11111 << 10, 0b11111 << 5, 0b11111, 0],
        32 => [0b11111111 << 16, 0b11111111 << 8, 0b11111111, 0],
        _ => [0, 0, 0, 0],
      },
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
        r8g8b8a8_Srgb::default()
      } else {
        r8g8b8a8_Srgb { r: 0, g: 0, b: 0, a: 0xFF }
      })
      .into(),
    );

    #[allow(unused_assignments)]
    let palette: Vec<r8g8b8a8_Srgb> = match info_header.palette_len() {
      0 => Vec::new(),
      count => {
        let mut v = Vec::new();
        v.try_reserve(count).map_err(|_| BmpError::AllocError)?;
        match info_header {
          BmpInfoHeader::Core(_) => {
            let bytes_needed = count * size_of::<[u8; 3]>();
            let (pal_slice, new_rest) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            rest = new_rest;
            let pal_slice: &[[u8; 3]] = cast_slice(pal_slice);
            for [b, g, r] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Srgb { r, g, b, a: 0xFF });
            }
          }
          _ => {
            let bytes_needed = count * size_of::<[u8; 4]>();
            let (pal_slice, new_rest) = if rest.len() < bytes_needed {
              return Err(BmpError::InsufficientBytes);
            } else {
              rest.split_at(bytes_needed)
            };
            rest = new_rest;
            let pal_slice: &[[u8; 4]] = cast_slice(pal_slice);
            for [b, g, r, a] in pal_slice.iter().copied() {
              v.push(r8g8b8a8_Srgb { r, g, b, a });
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
                  let p: P = r8g8b8a8_Srgb { r, g, b, a: 0xFF }.into();
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
                  let p: P = r8g8b8a8_Srgb { r, g, b, a }.into();
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
                  let p: P = r8g8b8a8_Srgb { r, g, b, a }.into();
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

    let bitmap = Bitmap { height: height as u32, width: width as u32, pixels: final_storage };
    Ok(bitmap)
  }
}

#[cfg(feature = "alloc")]
impl<P> crate::image::Palmap<u8, P>
where
  P: From<r8g8b8a8_Srgb> + Clone,
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
        let mut v: Vec<r8g8b8a8_Srgb> = Vec::new();
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
              v.push(r8g8b8a8_Srgb { r, g, b, a: 0xFF });
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
              v.push(r8g8b8a8_Srgb { r, g, b, a });
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
