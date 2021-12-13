//! Module for pixel formats.

use bytemuck::{Pod, Zeroable};

/// Grayscale, 8-bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y8 {
  pub y: u8,
}
unsafe impl Zeroable for Y8 {}
unsafe impl Pod for Y8 {}

/// Grayscale Alpha, 8-bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct YA8 {
  pub y: u8,
  pub a: u8,
}
unsafe impl Zeroable for YA8 {}
unsafe impl Pod for YA8 {}

/// RGB 8-bit per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGB8 {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}
unsafe impl Zeroable for RGB8 {}
unsafe impl Pod for RGB8 {}

/// RGBA 8-bit per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGBA8 {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}
unsafe impl Zeroable for RGBA8 {}
unsafe impl Pod for RGBA8 {}
