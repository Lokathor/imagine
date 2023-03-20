use super::*;

/// Exchangeable Image File (Exif) Profile
///
/// Spec: [eXIf](https://www.w3.org/TR/png/#eXIf)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct eXIf<'a> {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  #[cfg(not(feature = "alloc"))]
  data: &'a [u8],
  #[cfg(feature = "alloc")]
  data: alloc::borrow::Cow<'a, [u8]>,
  crc_claim: U32BE,
}
impl eXIf<'_> {
  /// View the exif data.
  ///
  /// The format of this data is well outside the PNG spec, and probably outside
  /// of what this crate cares about.
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
  pub fn to_owned(&self) -> eXIf<'static> {
    use alloc::borrow::ToOwned;
    eXIf {
      data: alloc::borrow::Cow::Owned(self.data.clone().into_owned()),
      chunk_ty: self.chunk_ty,
      crc_claim: self.crc_claim,
      length: self.length,
    }
  }
}
