use super::*;

/// Palette data
///
/// Palette entries are always RGB.
///
/// If you want to have a paletted image with transparency then the transparency
/// info goes in a separate transparency chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PLTE<'b>(&'b [[u8; 3]]);
impl<'b> From<&'b [[u8; 3]]> for PLTE<'b> {
  #[inline]
  #[must_use]
  fn from(entries: &'b [[u8; 3]]) -> Self {
    Self(entries)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for PLTE<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::PLTE(plte) => Ok(plte),
      _ => Err(()),
    }
  }
}
impl Debug for PLTE<'_> {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    // currently prints no more than 4 palette entries
    f.debug_tuple("PLTE").field(&&self.0[..self.0.len().min(4)]).field(&self.0.len()).finish()
  }
}
impl<'b> PLTE<'b> {
  /// Gets the entries as a slice.
  #[inline]
  pub fn entries(&self) -> &'b [[u8; 3]] {
    self.0
  }
}
