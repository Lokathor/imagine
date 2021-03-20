use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct HuffSymbol(usize);
// TODO: better debug?

#[test]
fn test_size_of_result_huff_symbol() {
  use core::mem::size_of;
  // Note(Lokathor): We want `PngResult<HuffSymbol>` to be two usize so that it
  // passes as two values in register instead of passing on the stack.
  assert_eq!(size_of::<PngResult<HuffSymbol>>(), size_of::<[usize; 2]>());
}

impl HuffSymbol {
  pub fn literal(lit: u8) -> Self {
    HuffSymbol(lit as usize)
  }
  pub fn get_literal(self) -> Option<u8> {
    if self.0 < 256 {
      Some(self.0 as u8)
    } else {
      None
    }
  }
  //
  pub fn end_of_block() -> Self {
    HuffSymbol(256)
  }
  pub fn is_end_of_block(self) -> bool {
    self.0 == 256
  }
  //
  pub fn back_ref(len: usize, dist: usize) -> Self {
    debug_assert!(len >= 3);
    debug_assert!(len <= 258);
    debug_assert!(dist >= 1);
    debug_assert!(dist <= 32_768);
    debug_assert!(u16::MAX >= 32_768);
    HuffSymbol(len << 16 | dist)
  }
  pub fn get_back_ref(self) -> (usize, usize) {
    (self.0 >> 16, self.0 & 0xFFFF)
  }
}
