use crate::borrowed_bitmap::*;
use pixel_formats::r8g8b8a8_Srgb;

/// An owned direct-color image.
///
/// The fields are public, but if you put them together weirdly the methods of
/// this type might panic.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub struct Bitmap<P = r8g8b8a8_Srgb> {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<P>,
}
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
impl<P> Bitmap<P> {
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
  /// Flips the image top to bottom.
  #[inline]
  pub fn vertical_flip(&mut self) {
    BorrowedBitmap { width: self.width, height: self.height, pixels: &mut self.pixels }
      .vertical_flip()
  }
}

/// An indexed-color image.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub struct Palmap<I = u8, P = r8g8b8a8_Srgb> {
  pub width: u32,
  pub height: u32,
  pub indexes: Vec<I>,
  pub palette: Vec<P>,
}
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
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
    BorrowedBitmap { width: self.width, height: self.height, pixels: &mut self.indexes }
      .vertical_flip()
  }
}

impl<I, PxIn, PxOut> From<&Palmap<I, PxIn>> for Bitmap<PxOut>
where
  usize: From<I>,
  I: Clone,
  PxOut: From<PxIn>,
  PxIn: Clone + Default,
{
  #[inline]
  fn from(palmap: &Palmap<I, PxIn>) -> Self {
    let pixels: Vec<PxOut> = palmap
      .indexes
      .iter()
      .cloned()
      .map(|i| PxOut::from(palmap.palette.get(usize::from(i)).cloned().unwrap_or_default()))
      .collect();
    Bitmap { width: palmap.width, height: palmap.height, pixels }
  }
}
