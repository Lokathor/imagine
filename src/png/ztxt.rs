use super::*;

/// Compressed textual data
///
/// Spec: [zTXt](https://www.w3.org/TR/png/#11zTXt)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct zTXt<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl zTXt<'_> {
  /// Chunk data.
  /// * keyword (should be 1-79 bytes)
  /// * null byte
  /// * compression method (only defined for zlib compression)
  /// * zlib compressed data stream
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
  pub fn to_owned(&self) -> zTXt<'static> {
    use alloc::borrow::ToOwned;
    zTXt {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
