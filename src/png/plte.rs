use super::*;

/// Palette
///
/// Spec: [PLTE](https://www.w3.org/TR/png/#11PLTE)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
  /// Palette entries.
  ///
  /// Each is an RGB value with 8 bits per channel. The PNG spec doesn't say,
  /// but the encoding scheme of the bytes (eg: gamma compressed or not)
  /// probably depends on other chunks within the PNG the same as how normal PNG
  /// pixel format colors work.
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

  /// Clone the data into a new, owned value.
  #[inline]
  #[must_use]
  #[cfg(feature = "alloc")]
  pub fn to_owned(&self) -> PLTE<'static> {
    use alloc::borrow::ToOwned;
    PLTE {
      entries: alloc::borrow::Cow::Owned(self.entries.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
