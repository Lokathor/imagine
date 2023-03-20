use super::*;

/// Coding-independent code points for video signal type identification
///
/// Spec: [cICP](https://www.w3.org/TR/png/#cICP-chunk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct cICP {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  color_primaries: u8,
  transfer_function: u8,
  matrix_coefficients: u8,
  video_full_range_flag: u8,
  crc_claim: U32BE,
}
