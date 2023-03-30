use crate::ascii_array::AsciiArray;
use pack1::*;

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
  pub x_pixels_per_meter: I32LE,
  pub y_pixels_per_meter: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
}

/// See [BitmapV5Header] for docs.
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
  pub x_pixels_per_meter: I32LE,
  pub y_pixels_per_meter: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
  pub r_mask: U32LE,
  pub g_mask: U32LE,
  pub b_mask: U32LE,
}

/// See [BitmapV5Header] for docs.
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
  pub x_pixels_per_meter: I32LE,
  pub y_pixels_per_meter: I32LE,
  pub colors_used: U32LE,
  pub important_colors: U32LE,
  pub r_mask: U32LE,
  pub g_mask: U32LE,
  pub b_mask: U32LE,
  pub a_mask: U32LE,
}

type FXPT2DOT30 = U32LE;
type CIEXYZ = [FXPT2DOT30; 3];
type CIEXYZTRIPLE = [CIEXYZ; 3];

/// See [BitmapV5Header] for docs.
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
  pub x_pixels_per_meter: I32LE,
  pub y_pixels_per_meter: I32LE,
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
