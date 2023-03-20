use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct pHYs {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  pixels_per_x: U32BE,
  pixels_per_y: U32BE,
  unit: u8,
  crc_claim: U32BE,
}
