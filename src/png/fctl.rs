use super::*;

/// Frame Control Chunk
///
/// Spec: [fcTL](https://www.w3.org/TR/png/#fcTL-chunk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct fcTL {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  sequence_number: U32BE,
  width: U32BE,
  height: U32BE,
  x_offset: U32BE,
  y_offset: U32BE,
  delay_num: U16BE,
  delay_den: U16BE,
  dispose_op: u8,
  blend_op: u8,
  crc_claim: U32BE,
}
