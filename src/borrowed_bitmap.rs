//! Provides heap-allocated image types.

use core::ops::{Index, IndexMut};

use alloc::vec::Vec;
use pixel_formats::r8g8b8a8_Srgb;

/// Converts an `(x,y)` position within a given `width` 2D space into a linear
/// index.
///
/// You don't ever need to call this function yourself, but it's how the image
/// containers convert 2d coordinates into index values within their payload
/// vectors. If you'd like to use the exact same function they do for some
/// reason, you can.
#[inline]
#[must_use]
pub const fn xy_width_to_index(x: u32, y: u32, width: u32) -> usize {
  y.wrapping_mul(width).wrapping_add(x) as usize
}

/// Borrow of bitmap data.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BorrowedBitmap<'a, P = r8g8b8a8_Srgb> {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Borrow of the pixel data.
  pub pixels: &'a mut [P],
}
impl<'a, P> BorrowedBitmap<'a, P> {
  /// Gets the pixel at the position, or `None` if the position is out of
  /// bounds.
  #[inline]
  #[must_use]
  pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut P> {
    if x < self.width && y < self.height {
      let i = xy_width_to_index(x, y, self.width);
      self.pixels.get_mut(i)
    } else {
      None
    }
  }
  /// Flips the image, top to bottom.
  ///
  /// If the buffer contains less pixels than `width * height` would indicate,
  /// this will do nothing.
  #[inline]
  pub fn vertical_flip(&mut self) {
    let num_pixels = self.width.wrapping_mul(self.height) as usize;
    if let Some(mut data) = self.pixels.get_mut(..num_pixels) {
      let mut temp_height = self.height;
      while temp_height > 1 {
        let (low, mid) = data.split_at_mut(self.width as usize);
        let (mid, high) = mid.split_at_mut(mid.len() - self.width as usize);
        low.swap_with_slice(high);
        data = mid;
        temp_height -= 2;
      }
    }
  }
}
