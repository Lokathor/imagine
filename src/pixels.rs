//! Module for various pixel data structures.

use bytemuck::{Pod, Zeroable};

/// Red/Green/Blue/Alpha, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
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

/// Alpha/Red/Green/Blue, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
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
impl From<RGBA8888> for ARGB8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, a }
  }
}

/// Alpha/Blue/Green/Red, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
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
impl From<RGBA8888> for ABGR8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, a }
  }
}

/// Ignored/Red/Green/Blue, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
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
impl From<RGBA8888> for XRGB8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, x: a }
  }
}

/// Ignored/Blue/Green/Red, u8 per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
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
impl From<RGBA8888> for XBGR8888 {
  #[inline]
  fn from(RGBA8888 { r, g, b, a }: RGBA8888) -> Self {
    Self { r, g, b, x: a }
  }
}
