#![forbid(unsafe_code)]

use crate::AsciiArray;
use core::num::{NonZeroU16, NonZeroU32};

pub struct BmpFileHeader {
  tag: AsciiArray<2>,
  file_size: u32,
  pixel_data_offset: u32,
}
impl From<[u8; 14]> for BmpFileHeader {
  #[inline]
  #[must_use]
  fn from(value: [u8; 14]) -> Self {
    Self {
      tag: AsciiArray(value[0..2].try_into().unwrap()),
      file_size: u32::from_le_bytes(value[2..6].try_into().unwrap()),
      pixel_data_offset: u32::from_le_bytes(value[10..14].try_into().unwrap()),
    }
  }
}
impl From<BmpFileHeader> for [u8; 14] {
  fn from(h: BmpFileHeader) -> Self {
    let mut a = [0; 14];
    a[0..2].copy_from_slice(h.tag.0.as_slice());
    a[2..6].copy_from_slice(h.file_size.to_le_bytes().as_slice());
    a[10..14].copy_from_slice(h.pixel_data_offset.to_le_bytes().as_slice());
    a
  }
}

/// Header for Windows 2.0 and OS/2 1.x images.
///
/// Unlikely to be seen in modern times.
///
/// Corresponds to the the 12 byte `BITMAPCOREHEADER` struct (aka
/// `OS21XBITMAPHEADER`).
pub struct BmpCoreHeader {
  /// Width in pixels
  width: i16,

  /// Height in pixels.
  ///
  /// In later versions of BMP, negative height means that the image origin is
  /// the top left and rows go down. Otherwise the origin is the bottom left,
  /// and rows go up. In this early version values are expected to always be
  /// positive, but if we do see a negative height here then probably we want to
  /// follow the same origin-flipping convention.
  height: i16,

  /// In this version of BMP, all colors are expected to be indexed, and this is
  /// the bits per index value (8 or less). An appropriate palette value should
  /// also be present in the bitmap.
  bits_per_pixel: u16,
}
impl TryFrom<[u8; 12]> for BmpCoreHeader {
  type Error = ();
  #[inline]
  fn try_from(a: [u8; 12]) -> Result<Self, Self::Error> {
    if u32::from_le_bytes(a[0..4].try_into().unwrap()) != 12 {
      return Err(());
    }
    if u16::from_le_bytes(a[8..10].try_into().unwrap()) != 1 {
      return Err(());
    }
    Ok(Self {
      width: i16::from_le_bytes(a[4..6].try_into().unwrap()),
      height: i16::from_le_bytes(a[6..8].try_into().unwrap()),
      bits_per_pixel: u16::from_le_bytes(a[10..12].try_into().unwrap()),
    })
  }
}
impl From<BmpCoreHeader> for [u8; 12] {
  #[inline]
  #[must_use]
  fn from(h: BmpCoreHeader) -> Self {
    let mut a = [0; 12];
    a[0..4].copy_from_slice(12_u32.to_le_bytes().as_slice());
    a[4..6].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[6..8].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[8..10].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[10..12].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a
  }
}

#[repr(transparent)]
pub struct BmpCompression(u32);

/// The basic modern bitmap header.
///
/// Corresponds to the 40 byte `BITMAPINFOHEADER` struct.
pub struct BmpInfoHeader {
  width: i32,
  height: i32,
  bits_per_pixel: u16,
  compression: BmpCompression,
  /// If `None`, the `compression` must be `RGB` (aka "no compression").
  image_size: Option<NonZeroU32>,
  horizontal_ppm: i32,
  vertical_ppm: i32,
  /// If `None`, defaults to `2**bits_per_pixel`.
  palette_size: Option<NonZeroU32>,
  /// If `None`, all colors are considered important.
  ///
  /// This is usually ignored either way.
  important_colors: Option<NonZeroU32>,
}
impl TryFrom<[u8; 40]> for BmpInfoHeader {
  type Error = ();
  #[inline]
  fn try_from(a: [u8; 40]) -> Result<Self, Self::Error> {
    if u32::from_le_bytes(a[0..4].try_into().unwrap()) != 40 {
      return Err(());
    }
    if u16::from_le_bytes(a[12..14].try_into().unwrap()) != 1 {
      return Err(());
    }
    Ok(Self {
      width: i32::from_le_bytes(a[4..8].try_into().unwrap()),
      height: i32::from_le_bytes(a[8..12].try_into().unwrap()),
      bits_per_pixel: u16::from_le_bytes(a[14..16].try_into().unwrap()),
      compression: BmpCompression(u32::from_le_bytes(a[16..20].try_into().unwrap())),
      image_size: NonZeroU32::new(u32::from_le_bytes(a[20..24].try_into().unwrap())),
      horizontal_ppm: i32::from_le_bytes(a[24..28].try_into().unwrap()),
      vertical_ppm: i32::from_le_bytes(a[28..32].try_into().unwrap()),
      palette_size: NonZeroU32::new(u32::from_le_bytes(a[32..36].try_into().unwrap())),
      important_colors: NonZeroU32::new(u32::from_le_bytes(a[36..40].try_into().unwrap())),
    })
  }
}
impl From<BmpInfoHeader> for [u8; 40] {
  #[inline]
  #[must_use]
  fn from(h: BmpInfoHeader) -> Self {
    let mut a = [0; 40];
    a[0..4].copy_from_slice(40_u32.to_le_bytes().as_slice());
    a[4..8].copy_from_slice(h.width.to_le_bytes().as_slice());
    a[8..12].copy_from_slice(h.height.to_le_bytes().as_slice());
    a[12..14].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[14..16].copy_from_slice(h.bits_per_pixel.to_le_bytes().as_slice());
    a[16..20].copy_from_slice(h.compression.0.to_le_bytes().as_slice());
    a[20..24]
      .copy_from_slice(h.image_size.map(NonZeroU32::get).unwrap_or(0).to_le_bytes().as_slice());
    a[24..28].copy_from_slice(h.horizontal_ppm.to_le_bytes().as_slice());
    a[28..32].copy_from_slice(h.vertical_ppm.to_le_bytes().as_slice());
    a[32..36]
      .copy_from_slice(h.palette_size.map(NonZeroU32::get).unwrap_or(0).to_le_bytes().as_slice());
    a[36..40].copy_from_slice(
      h.important_colors.map(NonZeroU32::get).unwrap_or(0).to_le_bytes().as_slice(),
    );
    a
  }
}

pub enum Halftoning {
  None,
  ErrorDiffusion { damping_percentage: u32 },
  Panda { x: u32, y: u32 },
  SuperCircle { x: u32, y: u32 },
}

/// A bitmap header from OS/2 era files, unlikely to be encountered.
///
/// Corresponds to the 64 byte `OS22XBITMAPHEADER`, which can also be presented
/// as a 16 byte header where all additional bytes are implied to be 0.
pub struct BmpOs22xHeader {
  width: i32,
  height: i32,
  bits_per_pixel: u16,
  compression: BmpCompression,
  /// If `None`, the `compression` must be `RGB` (aka "no compression").
  image_size: Option<NonZeroU32>,
  horizontal_ppm: i32,
  vertical_ppm: i32,
  /// If `None`, defaults to `2**bits_per_pixel`.
  palette_size: Option<NonZeroU32>,
  /// If `None`, all colors are considered important.
  ///
  /// This is usually ignored either way.
  important_colors: Option<NonZeroU32>,
}
impl TryFrom<[u8; 64]> for BmpOs22xHeader {
  type Error = ();
  fn try_from(a: [u8; 64]) -> Result<Self, Self::Error> {
    if u32::from_le_bytes(a[0..4].try_into().unwrap()) != 40 {
      return Err(());
    }
    if u16::from_le_bytes(a[12..14].try_into().unwrap()) != 1 {
      return Err(());
    }
    // resolution units
    // pixel_direction
    // color_table_encoding
    todo!()
  }
}

/*
Bitmap info header vN:

// v1
uint32 biSize;
int32 biWidth;
int32 biHeight;
uint16 biPlanes;
uint16 biBitCount;
uint32 biCompression;
uint32 biSizeImage;
uint32 biXPixelsPerMeter;
uint32 biYPixelsPerMeter;
uint32 biClrUsed;
uint32 biClrImportant;

// new v2
uint32 biRedMask;
uint32 biGreenMask;
uint32 biBlueMask;

// new  v3
uint32 biAlphaMask;

// new v4
DWORD        bV4CSType;
CIEXYZTRIPLE bV4Endpoints;
DWORD        bV4GammaRed;
DWORD        bV4GammaGreen;
DWORD        bV4GammaBlue;

// new v5
DWORD        bV5Intent;
DWORD        bV5ProfileData;
DWORD        bV5ProfileSize;
DWORD        bV5Reserved;
*/

/*
Extra Bit Masks
*/

/*
Color Table
*/

/*
Gap1
*/

/*
Pixel Array
*/

/*
Gap2
*/

/*
ICC Color Profile
*/
