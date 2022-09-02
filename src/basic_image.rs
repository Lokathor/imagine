#![forbid(unsafe_code)]

//! Provides a very basic image type just so that you can use the auto-decoders
//! from this crate.

use core::ops::{Index, IndexMut};

use alloc::vec::Vec;

/// A basic containers for [RGBA8] data.
///
/// * The `pixels` vec should hold `width * height` pixels, row by row. If you
///   make your own instance of this type with incorrect `width` and `height`
///   fields the accessor functions will give weird results and possibly panic
///   unexpectedly, so please don't.
/// * The struct takes no opinion on if the first row is the top or bottom of
///   the image, because different image formats and GPU libraries disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub struct Image<P> {
  /// Image width (in pixels).
  pub width: u32,
  /// Image height (in pixels).
  pub height: u32,
  /// Image pixel data.
  pub pixels: Vec<P>,
}
impl<P> Image<P> {
  /// Converts and `x` and `y` to an index into the `pixels` vec.
  ///
  /// ```txt
  /// index = y * width + x
  /// ```
  ///
  /// Does not perform bounds checks.
  ///
  /// You generally don't need to call this method yourself. However, it's whats
  /// used by other accessor methods of this type to generate an index into the
  /// `pixels` vector. If you want to use this for some sort of compatibility
  /// reason, you can.
  #[inline]
  #[must_use]
  pub const fn xy_to_index(&self, x: u32, y: u32) -> usize {
    ((y * self.width) + x) as usize
  }
  /// Gets a shared reference to the specified pixel.
  ///
  /// ## Failure
  /// * If `x` or `y` are out of bounds you get `None`.
  #[inline]
  #[must_use]
  pub fn get(&self, x: u32, y: u32) -> Option<&P> {
    if x >= self.width {
      return None;
    }
    if y >= self.height {
      return None;
    }
    self.pixels.get(self.xy_to_index(x, y))
  }
  /// Gets a unique reference to the specified pixel.
  ///
  /// ## Failure
  /// * If `x` or `y` are out of bounds you get `None`.
  #[inline]
  #[must_use]
  pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut P> {
    if x >= self.width {
      return None;
    }
    if y >= self.height {
      return None;
    }
    let i = self.xy_to_index(x, y);
    self.pixels.get_mut(i)
  }
}
impl<P> Index<(u32, u32)> for Image<P> {
  type Output = P;
  #[inline]
  #[must_use]
  #[track_caller]
  fn index(&self, (x, y): (u32, u32)) -> &Self::Output {
    assert!(x < self.width, "Desired X:{x} exceeds width:{}", self.width);
    assert!(y < self.height, "Desired Y:{x} exceeds height:{}", self.height);
    &self.pixels[self.xy_to_index(x, y)]
  }
}
impl<P> IndexMut<(u32, u32)> for Image<P> {
  #[inline]
  #[must_use]
  #[track_caller]
  fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut Self::Output {
    assert!(x < self.width, "Desired X:{x} exceeds width:{}", self.width);
    assert!(y < self.height, "Desired Y:{x} exceeds height:{}", self.height);
    let i = self.xy_to_index(x, y);
    &mut self.pixels[i]
  }
}
