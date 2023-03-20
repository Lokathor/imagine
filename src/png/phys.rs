use super::*;

/// Physical pixel dimensions
///
/// Spec: [pHYs](https://www.w3.org/TR/png/#11pHYs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct pHYs {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  pixels_per_x: U32BE,
  pixels_per_y: U32BE,
  unit: u8,
  crc_claim: U32BE,
}
