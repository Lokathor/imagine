use super::*;

/// Animation Control Chunk
///
/// Spec: [acTL](https://www.w3.org/TR/png/#acTL-chunk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct acTL {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  num_frames: U32BE,
  num_plays: U32BE,
  crc_claim: U32BE,
}
