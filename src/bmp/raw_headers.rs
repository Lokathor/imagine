#![allow(dead_code)]

//! These are the full header structs, but because there's so many versions we
//! don't want to expose this to users.

use crate::ascii_array::AsciiArray;
use pack1::*;

pub(super) type FXPT2DOT30 = U32LE;
pub(super) type CIEXYZ = [FXPT2DOT30; 3];
pub(super) type CIEXYZTRIPLE = [CIEXYZ; 3];
pub(super) const BI_RGB: u32 = 0;
pub(super) const BI_RLE8: u32 = 1;
pub(super) const BI_RLE4: u32 = 2;
pub(super) const BI_BITFIELDS: u32 = 3;
pub(super) const BI_ALPHABITFIELDS: u32 = 6;
pub(super) const LCS_GM_ABS_COLORIMETRIC: u32 = 0x00000008;
pub(super) const LCS_GM_BUSINESS: u32 = 0x00000001;
pub(super) const LCS_GM_GRAPHICS: u32 = 0x00000002;
pub(super) const LCS_GM_IMAGES: u32 = 0x00000004;
pub(super) const LCS_CALIBRATED_RGB: u32 = 0x00000000;
pub(super) const LCS_sRGB: u32 = 0x7352_4742;
pub(super) const LCS_WINDOWS_COLOR_SPACE: u32 = 0x5769_6E20;
pub(super) const PROFILE_LINKED: u32 = 0x4C49_4E4B;
pub(super) const PROFILE_EMBEDDED: u32 = 0x4D42_4544;

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapFileHeader {
  pub ty: AsciiArray<2>,
  pub file_size: U32LE,
  pub reserved1: U16LE,
  pub reserved2: U16LE,
  pub bitmap_offset: U32LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapCoreHeader {
  pub size: U32LE,
  pub width: U16LE,
  pub height: U16LE,
  pub planes: U16LE,
  pub bits_per_pixel: U16LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapInfoHeader {
  pub size: U32LE,
  pub width: I32LE,
  pub height: I32LE,
  pub planes: U16LE,
  pub bits_per_pixel: U16LE,
  pub compression: U32LE,
  pub image_size: U32LE,
  pub pixels_per_meter_x: I32LE,
  pub pixels_per_meter_y: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapV2InfoHeader {
  pub size: U32LE,
  pub width: I32LE,
  pub height: I32LE,
  pub planes: U16LE,
  pub bits_per_pixel: U16LE,
  pub compression: U32LE,
  pub image_size: U32LE,
  pub pixels_per_meter_x: I32LE,
  pub pixels_per_meter_y: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
  pub r_mask: U32LE,
  pub g_mask: U32LE,
  pub b_mask: U32LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapV3InfoHeader {
  pub size: U32LE,
  pub width: I32LE,
  pub height: I32LE,
  pub planes: U16LE,
  pub bits_per_pixel: U16LE,
  pub compression: U32LE,
  pub image_size: U32LE,
  pub pixels_per_meter_x: I32LE,
  pub pixels_per_meter_y: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
  pub r_mask: U32LE,
  pub g_mask: U32LE,
  pub b_mask: U32LE,
  pub a_mask: U32LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapV4Header {
  pub size: U32LE,
  pub width: I32LE,
  pub height: I32LE,
  pub planes: U16LE,
  pub bits_per_pixel: U16LE,
  pub compression: U32LE,
  pub image_size: U32LE,
  pub pixels_per_meter_x: I32LE,
  pub pixels_per_meter_y: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
  pub r_mask: U32LE,
  pub g_mask: U32LE,
  pub b_mask: U32LE,
  pub a_mask: U32LE,
  pub colorspace_type: U32LE,
  pub endpoints: CIEXYZTRIPLE,
  pub r_gamma: U32LE,
  pub g_gamma: U32LE,
  pub b_gamma: U32LE,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct BitmapV5Header {
  /// size of the struct.
  pub size: U32LE,

  /// width in pixels.
  pub width: I32LE,

  /// height in pixels. when height is positive the bitmap is bottom up. when
  /// the height is negative the bitmap is top down, and the `compression`
  /// should only be `BI_RGB` or `BI_BITFIELDS` (meaning it should *not* be
  /// `BI_RLE4` or `BI_RLE8`).
  pub height: I32LE,

  /// must be 1
  pub planes: U16LE,

  /// * 1, 4, 8: Paletted; possible RLE compression
  /// * 16: Direct; RGB`0bX_GGGGG_RRRRR_BBBBB` or Bitfields
  /// * 24: Direct; RGB`[b,g,r]`
  /// * 32: Direct; RGB`[b,g,r,x]` or Bitfields or AlphaBitfields
  pub bits_per_pixel: U16LE,

  /// * `BI_RGB`
  /// * `BI_RLE4`
  /// * `BI_RLE8`
  /// * `BI_BITFIELDS`
  /// * `BI_ALPHABITFIELDS`
  pub compression: U32LE,

  /// If non-zero, the size of the image. If the size is zero the compression
  /// must be `BI_RGB` and then the size is implied based on the number of
  /// pixels.
  pub image_size: U32LE,

  /// pixels per meter of the intended device, wide
  pub pixels_per_meter_x: I32LE,

  /// pixels per meter of the intended device, tall
  pub pixels_per_meter_y: I32LE,

  /// The number of color table entries that are used. If zero, uses the maximum
  /// number according to the bits per pixel.
  pub colors_used: U32LE,

  /// The number of colors that are important, if zero then all colors are
  /// important.
  pub important_colors: U32LE,

  /// Ignored unless `compression` is `BI_ALPHABITFIELDS` or `BI_BITFIELDS`
  pub r_mask: U32LE,

  /// Ignored unless `compression` is `BI_ALPHABITFIELDS` or `BI_BITFIELDS`
  pub g_mask: U32LE,

  /// Ignored unless `compression` is `BI_ALPHABITFIELDS` or `BI_BITFIELDS`
  pub b_mask: U32LE,

  /// Ignored unless `compression` is `BI_ALPHABITFIELDS`
  pub a_mask: U32LE,

  /// * `LCS_CALIBRATED_RGB`
  /// * `LCS_sRGB`
  /// * `LCS_WINDOWS_COLOR_SPACE`
  /// * `PROFILE_LINKED`
  /// * `PROFILE_EMBEDDED`
  pub colorspace_type: U32LE,

  /// Ignored unless `colorspace_type` is `LCS_CALIBRATED_RGB`
  pub endpoints: CIEXYZTRIPLE,

  /// Ignored unless `colorspace_type` is `LCS_CALIBRATED_RGB`
  pub r_gamma: U32LE,

  /// Ignored unless `colorspace_type` is `LCS_CALIBRATED_RGB`
  pub g_gamma: U32LE,

  /// Ignored unless `colorspace_type` is `LCS_CALIBRATED_RGB`
  pub b_gamma: U32LE,

  /// * `LCS_GM_ABS_COLORIMETRIC`
  /// * `LCS_GM_BUSINESS`
  /// * `LCS_GM_GRAPHICS`
  /// * `LCS_GM_IMAGES`
  pub render_intent: U32LE,

  /// Offset of the start of the color profile.
  pub color_profile_offset: U32LE,

  /// Size of the color profile.
  pub color_profile_size: U32LE,

  /// Should be zero.
  pub reserved: U32LE,
}

impl From<BitmapCoreHeader> for BitmapV5Header {
  fn from(
    BitmapCoreHeader { size, width, height, planes, bits_per_pixel }: BitmapCoreHeader,
  ) -> Self {
    Self {
      size,
      width: i32::from(width.get()).into(),
      height: i32::from(height.get()).into(),
      planes,
      bits_per_pixel,
      compression: BI_RGB.into(),
      image_size: 0.into(),
      pixels_per_meter_x: 0.into(),
      pixels_per_meter_y: 0.into(),
      colors_used: 0.into(),
      important_colors: 0.into(),
      r_mask: 0.into(),
      g_mask: 0.into(),
      b_mask: 0.into(),
      a_mask: 0.into(),
      colorspace_type: LCS_sRGB.into(),
      endpoints: [[0.into(); 3]; 3],
      r_gamma: 0.into(),
      g_gamma: 0.into(),
      b_gamma: 0.into(),
      render_intent: LCS_GM_IMAGES.into(),
      color_profile_offset: 0.into(),
      color_profile_size: 0.into(),
      reserved: 0.into(),
    }
  }
}

impl From<BitmapInfoHeader> for BitmapV5Header {
  fn from(
    BitmapInfoHeader {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
    }: BitmapInfoHeader,
  ) -> Self {
    Self {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask: 0.into(),
      g_mask: 0.into(),
      b_mask: 0.into(),
      a_mask: 0.into(),
      colorspace_type: LCS_sRGB.into(),
      endpoints: [[0.into(); 3]; 3],
      r_gamma: 0.into(),
      g_gamma: 0.into(),
      b_gamma: 0.into(),
      render_intent: LCS_GM_IMAGES.into(),
      color_profile_offset: 0.into(),
      color_profile_size: 0.into(),
      reserved: 0.into(),
    }
  }
}

impl From<BitmapV2InfoHeader> for BitmapV5Header {
  fn from(
    BitmapV2InfoHeader {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
    }: BitmapV2InfoHeader,
  ) -> Self {
    Self {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
      a_mask: 0.into(),
      colorspace_type: LCS_sRGB.into(),
      endpoints: [[0.into(); 3]; 3],
      r_gamma: 0.into(),
      g_gamma: 0.into(),
      b_gamma: 0.into(),
      render_intent: LCS_GM_IMAGES.into(),
      color_profile_offset: 0.into(),
      color_profile_size: 0.into(),
      reserved: 0.into(),
    }
  }
}

impl From<BitmapV3InfoHeader> for BitmapV5Header {
  fn from(
    BitmapV3InfoHeader {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
      a_mask,
    }: BitmapV3InfoHeader,
  ) -> Self {
    Self {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
      a_mask,
      colorspace_type: LCS_sRGB.into(),
      endpoints: [[0.into(); 3]; 3],
      r_gamma: 0.into(),
      g_gamma: 0.into(),
      b_gamma: 0.into(),
      render_intent: LCS_GM_IMAGES.into(),
      color_profile_offset: 0.into(),
      color_profile_size: 0.into(),
      reserved: 0.into(),
    }
  }
}

impl From<BitmapV4Header> for BitmapV5Header {
  fn from(
    BitmapV4Header {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
      a_mask,
      colorspace_type,
      endpoints,
      r_gamma,
      g_gamma,
      b_gamma,
    }: BitmapV4Header,
  ) -> Self {
    BitmapV5Header {
      size,
      width,
      height,
      planes,
      bits_per_pixel,
      compression,
      image_size,
      pixels_per_meter_x,
      pixels_per_meter_y,
      colors_used,
      important_colors,
      r_mask,
      g_mask,
      b_mask,
      a_mask,
      colorspace_type,
      endpoints,
      r_gamma,
      g_gamma,
      b_gamma,
      render_intent: LCS_GM_IMAGES.into(),
      color_profile_offset: 0.into(),
      color_profile_size: 0.into(),
      reserved: 0.into(),
    }
  }
}
