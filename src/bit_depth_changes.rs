/// Makes the bit depth of a channel be 8, with only integer operations.
///
/// The `B` value is how many bits are currently used within the source value
/// (other bits should be 0). The source depth can be either above or blow 8. If
/// the source depth is exactly 8 this is just a no op.
///
/// ## Panics
/// * If `B` is not in the range `1..=31`
#[inline]
#[must_use]
pub const fn int_make_depth_8<const B: u32>(src: u32) -> u8 {
  match B {
    1 => {
      if src != 0 {
        0b11111111_u8
      } else {
        0
      }
    }
    2 => ((src << 6) | (src << 4) | (src << 2) | src) as u8,
    3 => ((src << 5) | (src << 2) | (src >> 1)) as u8,
    4 => ((src << 4) | src) as u8,
    5 => ((src << 3) | (src >> 2)) as u8,
    6 => ((src << 2) | (src >> 4)) as u8,
    7 => ((src << 1) | (src >> 6)) as u8,
    8 => src as u8,
    9 => (src >> 1) as u8,
    10 => (src >> 2) as u8,
    11 => (src >> 3) as u8,
    12 => (src >> 4) as u8,
    13 => (src >> 5) as u8,
    14 => (src >> 6) as u8,
    15 => (src >> 7) as u8,
    16 => (src >> 8) as u8,
    17 => (src >> 9) as u8,
    18 => (src >> 10) as u8,
    19 => (src >> 11) as u8,
    20 => (src >> 12) as u8,
    21 => (src >> 13) as u8,
    22 => (src >> 14) as u8,
    23 => (src >> 15) as u8,
    24 => (src >> 16) as u8,
    25 => (src >> 17) as u8,
    26 => (src >> 18) as u8,
    27 => (src >> 19) as u8,
    28 => (src >> 20) as u8,
    29 => (src >> 21) as u8,
    30 => (src >> 22) as u8,
    31 => (src >> 23) as u8,
    _ => panic!("illegal source depth"),
  }
}
