use super::*;

/// Transparency data
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
pub struct tRNS<'b>(pub(crate) &'b [u8]);
impl<'b> From<&'b [u8]> for tRNS<'b> {
  #[inline]
  #[must_use]
  fn from(data: &'b [u8]) -> Self {
    Self(data)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for tRNS<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::tRNS(trns) => Ok(trns),
      _ => Err(()),
    }
  }
}
impl Debug for tRNS<'_> {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("tRNS").field(&self.0).field(&self.0.len()).finish()
  }
}
impl<'b> tRNS<'b> {
  /// Gets the grayscale value that is transparent.
  ///
  /// Fails when the chunk has the wrong length for grayscale.
  #[inline]
  pub const fn try_to_grayscale(&self) -> Option<u16> {
    match *self.0 {
      [y0, y1] => Some(u16::from_be_bytes([y0, y1])),
      _ => None,
    }
  }
  /// Gets the RGB value that is transparent.
  ///
  /// Fails when the chunk has the wrong length for rgb.
  #[inline]
  pub const fn try_to_rgb(&self) -> Option<[u16; 3]> {
    match *self.0 {
      [r0, r1, g0, g1, b0, b1] => Some([
        u16::from_be_bytes([r0, r1]),
        u16::from_be_bytes([g0, g1]),
        u16::from_be_bytes([b0, b1]),
      ]),
      _ => None,
    }
  }
  /// Gets the alpha values for each palette index.
  #[inline]
  pub const fn to_alphas(&self) -> &'b [u8] {
    self.0
  }
}
