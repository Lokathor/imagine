//! From the PNG spec:
//!
//! > Filters are applied to **bytes**, not to pixels, regardless of the bit
//! > depth or color type of the image.

use super::*;

pub fn reconstruct_in_place(temp_memory: &mut [u8], header: PngHeader) -> PngResult<()> {
  if header.interlace_method == PngInterlaceMethod::NO_INTERLACE {
    if header.filter_method != PngFilterMethod::ADAPTIVE {
      return Err(PngError::IllegalFilterMethod);
    }
    let bytes_per_scanline: usize = header.get_temp_memory_bytes_per_scanline()?;
    let h = header.height as usize;
    debug_assert!(bytes_per_scanline > 0);
    debug_assert!(h > 0);
    if bytes_per_scanline * h != temp_memory.len() {
      trace!("bps: {}, h: {}, temp_len: {}", bytes_per_scanline, h, temp_memory.len());
      return Err(PngError::TempMemoryWrongSizeForHeader);
    }
    let filter_chunk_size = header.get_filter_chunk_size()?;
    debug_assert!(filter_chunk_size > 0);
    let mut previous_pixel_line_data = &mut [][..];
    for scanline in temp_memory.chunks_exact_mut(bytes_per_scanline) {
      let (filter_byte, pixel_line_data) = scanline.split_first_mut().unwrap();
      debug_assert_eq!(pixel_line_data.len() % filter_chunk_size, 0);
      debug_assert!(pixel_line_data.len() > 0);
      match *filter_byte {
        0 => (),
        1 => {
          // unfilter with the value to the left (skip the first pixel).
          let mut pixel_line_iter = pixel_line_data.chunks_exact_mut(filter_chunk_size);
          let mut a_bytes = pixel_line_iter.next().unwrap();
          pixel_line_iter.for_each(|x_bytes| {
            for (x, a) in x_bytes.iter_mut().zip(a_bytes.iter()) {
              *x = reconstruct_sub(*x, *a);
            }
            a_bytes = x_bytes;
          });
        }
        2 => {
          // unfilter with the values up (skip the first line)
          if !previous_pixel_line_data.is_empty() {
            pixel_line_data.iter_mut().zip(previous_pixel_line_data.iter()).for_each(
              |(x, b)| {
                *x = reconstruct_up(*x, *b);
              },
            )
          }
        }
        3 => {
          // unfilter using both left and up
          if previous_pixel_line_data.is_empty() {
            // if there's no previous line we use all 0s
            let mut pixel_line_iter = pixel_line_data.chunks_exact_mut(filter_chunk_size);
            let mut a_bytes = pixel_line_iter.next().unwrap();
            pixel_line_iter.for_each(|x_bytes| {
              for (x, a) in x_bytes.iter_mut().zip(a_bytes.iter()) {
                *x = reconstruct_average(*x, *a, 0);
              }
              a_bytes = x_bytes;
            });
          } else {
            // when there's a previous line we use it, but in this case we can't
            // skip the first pixel so we set the a_bytes in a kinda funny way.
            let mut a_bytes = &[0; 16][..filter_chunk_size];
            let pixel_line_iter = pixel_line_data
              .chunks_exact_mut(filter_chunk_size)
              .zip(previous_pixel_line_data.chunks_exact(filter_chunk_size));
            pixel_line_iter.for_each(|(x_bytes, b_bytes)| {
              for ((x, a), b) in
                x_bytes.iter_mut().zip(a_bytes.iter()).zip(b_bytes.iter())
              {
                *x = reconstruct_average(*x, *a, *b);
              }
              a_bytes = x_bytes;
            });
          }
        }
        4 => {
          // unfilter using left, up, and up left
          if previous_pixel_line_data.is_empty() {
            // if there's no previous line we use all 0s
            let mut pixel_line_iter = pixel_line_data.chunks_exact_mut(filter_chunk_size);
            let mut a_bytes = pixel_line_iter.next().unwrap();
            pixel_line_iter.for_each(|x_bytes| {
              for (x, a) in x_bytes.iter_mut().zip(a_bytes.iter()) {
                *x = reconstruct_paeth(*x, *a, 0, 0);
              }
              a_bytes = x_bytes;
            });
          } else {
            // when there's a previous line we use it, but in this case we can't
            // skip the first pixel so we set the a_bytes and c_bytes in a kinda
            // funny way.
            let mut a_bytes = &[0; 16][..filter_chunk_size];
            let mut c_bytes = &[0; 16][..filter_chunk_size];
            let pixel_line_iter = pixel_line_data
              .chunks_exact_mut(filter_chunk_size)
              .zip(previous_pixel_line_data.chunks_exact(filter_chunk_size));
            pixel_line_iter.for_each(|(x_bytes, b_bytes)| {
              for ((x, a), (b, c)) in x_bytes
                .iter_mut()
                .zip(a_bytes.iter())
                .zip(b_bytes.iter().zip(c_bytes.iter()))
              {
                *x = reconstruct_paeth(*x, *a, *b, *c);
              }
              a_bytes = x_bytes;
              c_bytes = b_bytes;
            });
          }
        }
        _ => return Err(PngError::IllegalAdaptiveFilterType),
      }
      previous_pixel_line_data = pixel_line_data;
    }
    Ok(())
  } else {
    Err(PngError::InterlaceNotSupported)
  }
}

/// Reconstruct Filter Type 1
///
/// * `fx` filtered X
/// * `ra` reconstructed `a`:
///   * Bit Depth <8: the byte before this byte
///   * Bit Depth >=8: the corresponding byte from the pixel to the left of this
///     pixel (or skip reconstruction if this is the leftmost pixel)
const fn reconstruct_sub(fx: u8, ra: u8) -> u8 {
  fx.wrapping_add(ra)
}

/// Reconstruct Filter Type 2
///
/// * `fx` filtered X
/// * `rb` reconstructed `b`: The byte corresponding to this byte within the
///   previous scanline.
const fn reconstruct_up(fx: u8, rb: u8) -> u8 {
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
const fn reconstruct_average(fx: u8, ra: u8, rb: u8) -> u8 {
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
const fn reconstruct_paeth(fx: u8, ra: u8, rb: u8, rc: u8) -> u8 {
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
