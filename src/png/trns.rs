use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
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
