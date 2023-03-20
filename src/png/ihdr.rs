use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct IHDR {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  width: U32BE,
  height: U32BE,
  bit_depth: u8,
  color_type: u8,
  compression_method: u8,
  filter: u8,
  interlace_method: u8,
  crc_claim: U32BE,
}
