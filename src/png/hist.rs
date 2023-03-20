use super::*;

/// Image histogram
///
/// Spec: [hIST](https://www.w3.org/TR/png/#11hIST)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct hIST<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [U16BE],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [U16BE]>,
  crc_claim: U32BE,
}
impl hIST<'_> {
  /// histogram data
  #[inline]
  #[must_use]
  pub fn data(&self) -> &[U16BE] {
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
  pub fn to_owned(&self) -> hIST<'static> {
    use alloc::borrow::ToOwned;
    hIST {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
