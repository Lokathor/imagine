use super::*;

/// Background color
///
/// Spec: [bKGD](https://www.w3.org/TR/png/#11bKGD)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct bKGD<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl bKGD<'_> {
  /// Gets the data as a byte slice.
  ///
  /// The intended format of the slice depends on the PNG's color type:
  /// * Grayscale: [`U16BE`]
  /// * RGB: `[U16BE; 3]`
  /// * Indexed: `u8`
  #[inline]
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
  pub fn to_owned(&self) -> bKGD<'static> {
    use alloc::borrow::ToOwned;
    bKGD {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
