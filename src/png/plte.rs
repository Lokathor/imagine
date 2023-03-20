use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PLTE<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  entries: &'a [[u8; 3]],
  #[cfg(feature = "alloc")]
  entries: alloc::borrow::Cow<'a, [[u8; 3]]>,
  crc_claim: U32BE,
}
impl PLTE<'_> {
  #[inline]
  pub fn entries(&self) -> &[[u8; 3]] {
    #[cfg(not(feature = "alloc"))]
    {
      self.entries
    }
    #[cfg(feature = "alloc")]
    {
      &self.entries
    }
  }
}
