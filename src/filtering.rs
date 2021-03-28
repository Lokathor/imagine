//! From the PNG spec:
//!
//! > Filters are applied to **bytes**, not to pixels, regardless of the bit
//! > depth or color type of the image.

use super::*;

/// Reconstruct Filter Type 1
///
/// * `fx` filtered X
/// * `ra` reconstructed `a`:
///   * Bit Depth <8: the byte before this byte
///   * Bit Depth >=8: the corresponding byte from the pixel to the left of this
///     pixel (or skip reconstruction if this is the leftmost pixel)
pub const fn reconstruct_sub(fx: u8, ra: u8) -> u8 {
  fx.wrapping_add(ra)
}

/// Reconstruct Filter Type 2
///
/// * `fx` filtered X
/// * `rb` reconstructed `b`: The byte corresponding to this byte within the
///   previous scanline.
pub const fn reconstruct_up(fx: u8, rb: u8) -> u8 {
  fx.wrapping_add(rb)
}

/// Reconstruct Filter Type 3
///
/// * `fx` filtered X
/// * `ra` reconstructed `a`:
///   * Bit Depth <8: the byte before this byte
///   * Bit Depth >=8: the corresponding byte from the pixel to the left of this
///     pixel (or skip reconstruction if this is the leftmost pixel)
/// * `rb` reconstructed `b`: The byte corresponding to this byte within the
///   previous scanline.
pub const fn reconstruct_average(fx: u8, ra: u8, rb: u8) -> u8 {
  fx.wrapping_add(ra.wrapping_add(rb).wrapping_div(2))
}

/// Reconstruct Filter Type 4
///
/// * `fx` filtered X
/// * `ra` reconstructed `a`:
///   * Bit Depth <8: the byte before this byte
///   * Bit Depth >=8: the corresponding byte from the pixel to the left of this
///     pixel (or skip reconstruction if this is the leftmost pixel)
/// * `rb` reconstructed `b`: The byte corresponding to this byte within the
///   previous scanline.
/// * `rc` reconstructed `c`:
pub const fn reconstruct_paeth(fx: u8, ra: u8, rb: u8, rc: u8) -> u8 {
  fx.wrapping_add(paeth_predictor(ra, rb, rc))
}

/// The Paeth filter function computes a simple linear function of the three
/// neighboring pixels (left `a`, above `b`, upper left `c`).
///
/// The output is the "predictor" of the neighboring pixel closest to the
/// computed value.
///
/// If any neighboring pixel isn't present because this is the top or left edge
/// of the image just substitute 0 in that postition.
const fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
  // Note(Lokathor): PNG spec says "The calculations within the PaethPredictor
  // function shall be performed exactly, without overflow.", so we use i32 math
  // here, which is wide enough to never give us trouble no matter what the u8
  // input values are.
  let a = a as i32;
  let b = b as i32;
  let c = c as i32;
  let p = a.wrapping_add(b).wrapping_sub(c);
  let pa = p.wrapping_sub(a).wrapping_abs();
  let pb = p.wrapping_sub(b).wrapping_abs();
  let pc = p.wrapping_sub(c).wrapping_abs();
  if pa <= pb && pa <= pc {
    a as u8
  } else if pb <= pc {
    b as u8
  } else {
    c as u8
  }
}
