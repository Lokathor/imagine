use super::*;
use crate::util::*;

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
