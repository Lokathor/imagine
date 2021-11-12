//#![no_std]
#![allow(unused_imports)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(target_pointer_width = "16")]
compile_error!("this crate requires 32-bit or bigger pointers!");

pub mod png;

pub type RGBA8 = [u8; 4];

pub type ImageRGBA8 = Image<RGBA8>;

pub struct Image<P> {
  width: u32,
  height: u32,
  pixels: Vec<P>,
}

impl<P> Image<P> {
  #[inline]
  #[must_use]
  pub const fn width(&self) -> u32 {
    self.width
  }
  #[inline]
  #[must_use]
  pub const fn height(&self) -> u32 {
    self.height
  }
  #[inline]
  #[must_use]
  pub fn pixels(&self) -> &[P] {
    self.pixels.as_slice()
  }
}
