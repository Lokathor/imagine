//! Module for pixel formats.
//!
//! ## Color Space
//!
//! The data types here are fairly plain containers for color data, and
//! particularly they are not associated with a particular color space.
//!
//! There are two color spaces that are commonly used (though others also
//! exist):
//! * Linear
//! * [sRGB](https://en.wikipedia.org/wiki/SRGB) (with linear alpha, if any).
//!
//! Sometimes an image will be explicitly marked as being of a specific color
//! space in its metadata. If it's not given, usually you can still assume that
//! `u8` channel data is in the `sRGB` color space. Linear data usually needs
//! more than 8 bits of precision for dark colors to look good, so linear data
//! is usually `f32` values (very occasionally `u16`).
//!
//! Color blending, and most other color operations, will only work correctly
//! with a linear color space. If you have sRGB values then you must convert
//! them to linear before you perform the operation.
//!
//! Converting sRGB to Linear is as follows:
//! ```
//! # let srgb = 0_u8;
//! let s = (srgb as f32) / 255.0;
//! let l = if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).pow(2.4) };
//! ```
//!
//! * If you want to speed things up with a slight loss in accuracy, the second
//!   step can be replaced with a plain `s.pow(2.2)`.
//! * If you *really* want to go for speed over accuracy you can just plain
//!   square the value (which is like `.pow(2.0)`).
//!
//! Converting linear values back to sRGB just requires that you reverse the
//! equation.
//! ```
//! # let l = 0.0_f32;
//! let s = (if l <= 0.0031308 {
//!   l * 12.92
//! } else {
//!   l.pow(1.0/2.4) * 1.055 − 0.055
//! } * 255.0) as u8;
//! ```
//!
//! Or, if you used an approximation:
//! * Raise to the power of `1.0/2.2` to invert raising to the power of `2.2`.
//! * Do a `sqrt` to invert the value being squared.
//!
//! ## Pre-multiplied Alpha
//!
//! TODO

use bytemuck::{Pod, Zeroable};

/// Grayscale, `u8`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y8 {
  pub y: u8,
}
unsafe impl Zeroable for Y8 {}
unsafe impl Pod for Y8 {}

/// Grayscale plus Alpha, `u8`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct YA8 {
  pub y: u8,
  pub a: u8,
}
unsafe impl Zeroable for YA8 {}
unsafe impl Pod for YA8 {}

/// Red/Green/Blue, `u8`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGB8 {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}
unsafe impl Zeroable for RGB8 {}
unsafe impl Pod for RGB8 {}

/// Red/Green/Blue plus Alpha, `u8`.
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
