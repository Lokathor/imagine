use super::*;
use crate::util::*;

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
