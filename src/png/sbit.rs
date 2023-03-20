use super::*;

/// Significant bits
///
/// Spec: [sBIT](https://www.w3.org/TR/png/#11sBIT)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct sBIT<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl sBIT<'_> {
  /// Chunk data. There's one byte per channel (according to the PNG's pixel
  /// format), giving the number of significant bits for each channel:
  /// * grayscale
  /// * RGB
  /// * grayscale + alpha
  /// * RGBA
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
  pub fn to_owned(&self) -> sBIT<'static> {
    use alloc::borrow::ToOwned;
    sBIT {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
