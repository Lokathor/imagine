use super::*;

/// Frame Data Chunk
///
/// Spec: [fdAT](https://www.w3.org/TR/png/#fdAT-chunk)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct fdAT<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  sequence_number: U32BE,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl fdAT<'_> {
  /// frame data
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
  pub fn to_owned(&self) -> fdAT<'static> {
    use alloc::borrow::ToOwned;
    fdAT {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      sequence_number: self.sequence_number,
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
