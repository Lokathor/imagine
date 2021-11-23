//! Module for pixel formats.
//!
//! There's two main factors with a pixel format:
//! * **Channels:** generally one or more of red, green, blue, and alpha. Some
//!   formats only use gray, marked as "Y" in format names. Other channel
//!   combinations also exist.
//! * **Bit Depth:** how many bits per channel. All current formats have
//!   identical bit depth for all channels, but formats do exist with different
//!   depths per channel.
//!
//! When the bit depth and channel layout allows, multiple pixels can be packed
//! within a single byte.
//!
//! **Note:** All of the current formats are what's required for PNG support.
//! Other formats might be added in the future as more image formats are added.
//!
//! ## Format Conversion
//!
//! Since pixel formats have two main factors (channels and depth), there's two
//! ways that you might change data between pixel formats.
//!
//! ### Between Gray and RGB
//! When going between grayscale and RGB coloring, the simple method is to use
//! an even split (to RGB) or an average (to Gray).
//!
//! However, human eyes don't respond equally to all three colors. To account
//! for this, you can weight the value of each channel.
//! ```text
//! Y = 0.299 * R + 0.587 * G + 0.114 * B
//! ```
//!
//! ### Between Bit Depths
//! All current formats have channel values stored only as integer values. Even
//! so, there's more than one way to convert between bit depths.
//!
//! * If sticking with integers: to *reduce* bit depth just keep the top X many
//!   bits, and to *increase* bit depth you should use the current bit pattern
//!   as the top X many bits, and then copy that bit pattern down however many
//!   times is required to fill in all newly added bits.
//! * Alternately, you can use floats: in this case, increaseing or decreasing
//!   the bit depth uses the same system. Convert the integer value to a float
//!   and divide by the maximum value of the starting bit depth (giving a
//!   normalized value), then multiply by the maximum of the target bit depth,
//!   and convert back to an integer.

use bytemuck::{Pod, Zeroable};

/// Eight 1-bit greyscale pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y1x8 {
  y: u8,
}
/// Four 2-bit greyscale pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y2x4 {
  y: u8,
}
/// Two 4-bit greyscale pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y4x2 {
  y: u8,
}
/// An 8-bit greyscale pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y8 {
  y: u8,
}
/// A 16-bit greyscale pixel.
///
/// The data is stored as a two-byte array (big-endian) to keep the type's
/// overall alignment at only 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Y16_BE {
  y: [u8; 2],
}

/// An RGB value, 8-bits per channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGB8 {
  r: u8,
  g: u8,
  b: u8,
}
/// An RGB value, 16-bits per channel.
///
/// The data is stored as a two-byte array (big-endian) to keep the type's
/// overall alignment at only 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGB16_BE {
  r: [u8; 2],
  g: [u8; 2],
  b: [u8; 2],
}

/// Eight 1-bit indexd pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Index1x8 {
  i: u8,
}
/// Four 2-bit indexed pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Index2x4 {
  i: u8,
}
/// Two 4-bit indexed pixels, tightly packed.
///
/// The high bits are the leftmost packed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Index4x2 {
  i: u8,
}
/// An 8-bit indexed pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Index8 {
  i: u8,
}

/// An 8-bits per channel greyscale + alpha pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct YA8 {
  y: u8,
  a: u8,
}
/// A 16-bits per channel greyscale + alpha pixel.
///
/// The data is stored as a two-byte array (big-endian) to keep the type's
/// overall alignment at only 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct YA16_BE {
  y: [u8; 2],
  a: [u8; 2],
}

/// An 8-bits per channel RGBA pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGBA8 {
  r: u8,
  g: u8,
  b: u8,
  a: u8,
}
/// A 16-bits per channel RGBA pixel.
///
/// The data is stored as a two-byte array (big-endian) to keep the type's
/// overall alignment at only 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RGBA16_BE {
  r: [u8; 2],
  g: [u8; 2],
  b: [u8; 2],
  a: [u8; 2],
}

unsafe impl Zeroable for Y1x8 {}
unsafe impl Zeroable for Y2x4 {}
unsafe impl Zeroable for Y4x2 {}
unsafe impl Zeroable for Y8 {}
unsafe impl Zeroable for Y16_BE {}
unsafe impl Zeroable for RGB8 {}
unsafe impl Zeroable for RGB16_BE {}
unsafe impl Zeroable for Index1x8 {}
unsafe impl Zeroable for Index2x4 {}
unsafe impl Zeroable for Index4x2 {}
unsafe impl Zeroable for Index8 {}
unsafe impl Zeroable for YA8 {}
unsafe impl Zeroable for YA16_BE {}
unsafe impl Zeroable for RGBA8 {}
unsafe impl Zeroable for RGBA16_BE {}
//
unsafe impl Pod for Y1x8 {}
unsafe impl Pod for Y2x4 {}
unsafe impl Pod for Y4x2 {}
unsafe impl Pod for Y8 {}
unsafe impl Pod for Y16_BE {}
unsafe impl Pod for RGB8 {}
unsafe impl Pod for RGB16_BE {}
unsafe impl Pod for Index1x8 {}
unsafe impl Pod for Index2x4 {}
unsafe impl Pod for Index4x2 {}
unsafe impl Pod for Index8 {}
unsafe impl Pod for YA8 {}
unsafe impl Pod for YA16_BE {}
unsafe impl Pod for RGBA8 {}
unsafe impl Pod for RGBA16_BE {}
