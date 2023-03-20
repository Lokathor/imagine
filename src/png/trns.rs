use super::*;

/// Transparency
///
/// Spec: [tRNS](https://www.w3.org/TR/png/#11tRNS)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct tRNS<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl tRNS<'_> {
  /// Transparency data. Format depends on the PNG color type:
  /// * greyscale: [`U16BE`]
  /// * RGB: `[U16BE; 3]`
  /// * indexed: `&[u8]` of alpha data that should be paired with the palette
  ///   entries. There can be less alpha entries than palette entries (default
  ///   missing entries to full opacity, `0xFF`)
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
  pub fn to_owned(&self) -> tRNS<'static> {
    use alloc::borrow::ToOwned;
    tRNS {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
