use super::*;

/// Standard RGB colour space
///
/// Spec: [sRGB](https://www.w3.org/TR/png/#11sRGB)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct sRGB {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  rendering_intent: u8,
  crc_claim: U32BE,
}
