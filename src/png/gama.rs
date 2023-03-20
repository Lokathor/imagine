use super::*;

/// Image gamma
///
/// Spec: [gAMA](https://www.w3.org/TR/png/#11gAMA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct gAMA {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  gamma: U32BE,
  crc_claim: U32BE,
}
