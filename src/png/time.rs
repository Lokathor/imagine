use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct tIME {
  length: U32BE,
  chunk_ty: AsciiArray<4>,
  year: U16BE,
  month: u8,
  day: u8,
  hour: u8,
  minute: u8,
  second: u8,
  crc_claim: U32BE,
}
