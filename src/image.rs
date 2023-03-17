#![forbid(unsafe_code)]

//! Provides heap-allocated image types.

use core::ops::{Index, IndexMut};

use alloc::vec::Vec;

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
  (y * width + x) as usize
}

/// A direct-color image.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub struct Bitmap<P> {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<P>,
}
impl<P> Bitmap<P> {
  /// Gets the pixel at the position, or `None` if the position is out of
  /// bounds.
  #[inline]
  #[must_use]
  pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut P> {
    if x < self.width && y < self.height {
      let i = xy_width_to_index(x, y, self.width);
      Some(&mut self.pixels[i])
    } else {
      None
    }
  }

  /// Flips the image top to bottom.
  #[inline]
  pub fn vertical_flip(&mut self) {
    let mut data: &mut [P] = self.pixels.as_mut_slice();
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

/// An indexed-color image.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub struct Palmap<I, P> {
  pub width: u32,
  pub height: u32,
  pub indexes: Vec<I>,
  pub palette: Vec<P>,
}
impl<I, P> Palmap<I, P> {
  /// Gets the index at the position, or `None` if the position is out of
  /// bounds.
  #[inline]
  #[must_use]
  pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut I> {
    if x < self.width && y < self.height {
      let i = xy_width_to_index(x, y, self.width);
      Some(&mut self.indexes[i])
    } else {
      None
    }
  }

  /// Flips the image top to bottom.
  #[inline]
  pub fn vertical_flip(&mut self) {
    let mut data: &mut [I] = self.indexes.as_mut_slice();
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

impl<I, P, B> From<&Palmap<I, P>> for Bitmap<B>
where
  usize: From<I>,
  I: Clone,
  B: From<P>,
  P: Clone + Default,
{
  #[inline]
  fn from(palmap: &Palmap<I, P>) -> Self {
    let pixels: Vec<B> = palmap
      .indexes
      .iter()
      .cloned()
      .map(|i| {
        let pal_index = usize::from(i);
        let color = palmap.palette.get(pal_index).cloned().unwrap_or_default();
        B::from(color)
      })
      .collect();
    Bitmap { width: palmap.width, height: palmap.height, pixels }
  }
}
