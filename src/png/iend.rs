use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct IEND {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  crc_claim: U32BE,
}
impl Default for IEND {
  #[inline]
  #[must_use]
  fn default() -> Self {
    let mut out = Self {
      length: U32BE::from_u32(size_of!(IEND)),
      chunk_ty: AsciiArray(*b"IEND"),
      crc_claim: Default::default(),
    };
    out.crc_claim = U32BE::from_u32(out.compute_crc());
    out
  }
}
impl IEND {
  #[inline]
  #[must_use]
  pub fn compute_crc(&self) -> u32 {
    png_crc(self.chunk_ty.as_bytes().iter().copied())
  }
}
