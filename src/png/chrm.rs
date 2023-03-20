use super::*;

/// Primary chromaticities and white point
///
/// Spec: [cHRM](https://www.w3.org/TR/png/#11cHRM)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct cHRM {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  white_point_x: U32BE,
  white_point_y: U32BE,
  red_x: U32BE,
  red_y: U32BE,
  green_x: U32BE,
  green_y: U32BE,
  blue_x: U32BE,
  blue_y: U32BE,
  crc_claim: U32BE,
}
