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
