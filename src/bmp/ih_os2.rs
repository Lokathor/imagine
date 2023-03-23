use super::*;
use crate::util::*;

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
