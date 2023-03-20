use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct gAMA {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  gamma: U32BE,
  crc_claim: U32BE,
}
