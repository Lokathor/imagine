use super::*;

/// Suggested palette
///
/// Spec: [sPLT](https://www.w3.org/TR/png/#11sPLT)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct sPLT<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl sPLT<'_> {
  /// Suggested palette data, with a small header and then a series of chunks.
  /// Each chunk is either six or ten bytes, depending on the bit depth of the
  /// RGBA channels (1 byte each or 2 bytes each).
  #[inline]
  #[must_use]
  pub fn data(&self) -> &[u8] {
    #[cfg(not(feature = "alloc"))]
    {
      self.data
    }
    #[cfg(feature = "alloc")]
    {
      &self.data
    }
  }

  /// Clone the data into a new, owned value.
  #[inline]
  #[must_use]
  #[cfg(feature = "alloc")]
  pub fn to_owned(&self) -> sPLT<'static> {
    use alloc::borrow::ToOwned;
    sPLT {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
