use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
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
