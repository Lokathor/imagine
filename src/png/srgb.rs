use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct sRGB {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  rendering_intent: u8,
  crc_claim: U32BE,
}
