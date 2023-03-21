use super::*;
/// Image Data.
///
/// * Image data is stored with Zlib compression applied.
/// * Images can have more than one IDAT chunk. They should all be stored in a
///   row. Multiple chunks are treated as a single Zlib datastream.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IDAT<'b>(&'b [u8]);
impl<'b> From<&'b [u8]> for IDAT<'b> {
  #[inline]
  #[must_use]
  fn from(data: &'b [u8]) -> Self {
    Self(data)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for IDAT<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::IDAT(idat) => Ok(idat),
      _ => Err(()),
    }
  }
}
impl Debug for IDAT<'_> {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("IDAT").field(&&self.0[..self.0.len().min(12)]).field(&self.0.len()).finish()
  }
}
impl<'b> IDAT<'b> {
  #[inline]
  #[must_use]
  pub(crate) fn as_bytes(&self) -> &'b [u8] {
    self.0
  }
}
