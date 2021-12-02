use crate::AsciiArray;
use core::num::{NonZeroU16, NonZeroU32};

pub struct BmpFileHeader {
  tag: AsciiArray<2>,
  file_size: u32,
  pixel_data_offset: u32,
}

/*
== DIB Header: one of any of the following structs:
* OS22XBITMAPHEADER (64)
* OS22XBITMAPHEADER (16 byte variant)
* BITMAPINFOHEADER (40)
* BITMAPV2INFOHEADER (52)
* BITMAPV3INFOHEADER (56)
* BITMAPV4HEADER (108)
* BITMAPV5HEADER (124)
*/

/// Header for Windows 2.0 and OS/2 1.x images.
///
/// Unlikely to be seen in modern times.
///
/// Corresponds to the the 12 byte `BITMAPCOREHEADER` struct (aka
/// `OS21XBITMAPHEADER`).
pub struct BitmapCoreHeader {
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
impl TryFrom<[u8; 12]> for BitmapCoreHeader {
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
impl From<BitmapCoreHeader> for [u8; 12] {
  #[inline]
  #[must_use]
  fn from(header: BitmapCoreHeader) -> Self {
    let mut a = [0; 12];
    a[0..4].copy_from_slice(12_u32.to_le_bytes().as_slice());
    a[4..6].copy_from_slice(header.width.to_le_bytes().as_slice());
    a[6..8].copy_from_slice(header.height.to_le_bytes().as_slice());
    a[8..10].copy_from_slice(1_u16.to_le_bytes().as_slice());
    a[10..12].copy_from_slice(header.bits_per_pixel.to_le_bytes().as_slice());
    a
  }
}

// TODO: OS22XBITMAPHEADER, 64 or 16 bytes

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
