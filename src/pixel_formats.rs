//! Module for various pixel data structures.

use bytemuck::{Pod, Zeroable};

/// sRGB Intent for an image.
///
/// Unless you're able to color correct, the exact value doesn't really matter.
/// However, knowing that image data is sRGB or not *at all* can be helpful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
#[allow(missing_docs)]
pub enum sRGBIntent {
  Perceptual,
  RelativeColorimetric,
  Saturation,
  AbsoluteColorimetric,
}

/// Red/Green/Blue/Alpha, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[cfg_attr(feature = "align_pixels", repr(align(4)))]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct RGBA8888 {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}
unsafe impl Zeroable for RGBA8888 {}
unsafe impl Pod for RGBA8888 {}
impl From<RGB888> for RGBA8888 {
  /// Gives an alpha value of `0xFF`
  #[inline]
  #[must_use]
  fn from(RGB888 { r, g, b }: RGB888) -> Self {
    Self { r, g, b, a: 0xFF }
  }
}
impl RGBA8888 {
  /// Black, RGB all 0, full alpha.
  pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 0xFF };
  /// White, all channels full.
  pub const WHITE: Self = Self { r: 0xFF, g: 0xFF, b: 0xFF, a: 0xFF };
}

/// Alpha/Red/Green/Blue, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[cfg_attr(feature = "align_pixels", repr(align(4)))]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct ARGB8888 {
  pub a: u8,
  pub r: u8,
  pub g: u8,
  pub b: u8,
}
unsafe impl Zeroable for ARGB8888 {}
unsafe impl Pod for ARGB8888 {}
impl From<RGBA8888> for ARGB8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, a }
  }
}

/// Alpha/Blue/Green/Red, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[cfg_attr(feature = "align_pixels", repr(align(4)))]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct ABGR8888 {
  pub a: u8,
  pub b: u8,
  pub g: u8,
  pub r: u8,
}
unsafe impl Zeroable for ABGR8888 {}
unsafe impl Pod for ABGR8888 {}
impl From<RGBA8888> for ABGR8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, a }
  }
}

/// Ignored/Red/Green/Blue, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[cfg_attr(feature = "align_pixels", repr(align(4)))]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct XRGB8888 {
  pub x: u8,
  pub r: u8,
  pub g: u8,
  pub b: u8,
}
unsafe impl Zeroable for XRGB8888 {}
unsafe impl Pod for XRGB8888 {}
impl From<RGBA8888> for XRGB8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, x: a }
  }
}

/// Ignored/Blue/Green/Red, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[cfg_attr(feature = "align_pixels", repr(align(4)))]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct XBGR8888 {
  pub x: u8,
  pub b: u8,
  pub g: u8,
  pub r: u8,
}
unsafe impl Zeroable for XBGR8888 {}
unsafe impl Pod for XBGR8888 {}
impl From<RGBA8888> for XBGR8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, x: a }
  }
}

/// Red/Green/Blue, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[allow(missing_docs)]
#[cfg(target_endian = "little")]
pub struct RGB888 {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}
unsafe impl Zeroable for RGB888 {}
unsafe impl Pod for RGB888 {}
