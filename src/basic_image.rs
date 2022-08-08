//! Provides a very basic image type just so that you can use the auto-decoders
//! from this crate.

use alloc::vec::Vec;

pub struct ImageRGBA {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<[u8; 4]>,
}
