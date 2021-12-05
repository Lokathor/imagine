use super::*;

use core::iter::repeat;

// FIXME: maybe we should branch on the header format's bits_per_channel
// outside of the function and pick one of several closures, then we can skip
// that branch per pixel. However, probably in practice the branch predictor
// doesn't care too much, I hope.
fn send_out_pixel<F: FnMut(u32, u32, &[u8])>(
  header: IHDR, image_level: usize, reduced_x: u32, reduced_y: u32, data: &[u8], op: &mut F,
) {
  match header.pixel_format.bits_per_channel() {
    1 => {
      let full_data: u8 = data[0];
      let mut mask = 0b1000_0000;
      let mut down_shift = 7;
      for plus_x in 0..8 {
        let (image_x, image_y) =
          interlaced_pos_to_full_pos(image_level, reduced_x * 8 + plus_x, reduced_y);
        op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
        mask >>= 1;
        down_shift -= 1;
      }
    }
    2 => {
      let full_data: u8 = data[0];
      let mut mask = 0b1100_0000;
      let mut down_shift = 6;
      for plus_x in 0..4 {
        let (image_x, image_y) =
          interlaced_pos_to_full_pos(image_level, reduced_x * 4 + plus_x, reduced_y);
        op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
        mask >>= 2;
        down_shift -= 2;
      }
    }
    4 => {
      let full_data: u8 = data[0];
      let mut mask = 0b1111_0000;
      let mut down_shift = 4;
      for plus_x in 0..2 {
        let (image_x, image_y) =
          interlaced_pos_to_full_pos(image_level, reduced_x * 2 + plus_x, reduced_y);
        op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
        mask >>= 4;
        down_shift -= 4;
      }
    }
    8 | 16 => {
      let (image_x, image_y) = interlaced_pos_to_full_pos(image_level, reduced_x, reduced_y);
      op(image_x as u32, image_y as u32, data);
    }
    _ => unreachable!(),
  }
}

const fn paeth_predict(a: u8, b: u8, c: u8) -> u8 {
  let a_ = a as i32;
  let b_ = b as i32;
  let c_ = c as i32;
  let p: i32 = a_ + b_ - c_;
  let pa = (p - a_).abs();
  let pb = (p - b_).abs();
  let pc = (p - c_).abs();
  // Note(Lokathor): The PNG spec is extremely specific that you shall not,
  // under any circumstances, alter the order of evaluation of this
  // expression's tests.
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
}

/// Given the `header`, `decompressed` buffer, and a per-pixel `op`, unfilters
/// the data and passes each pixel output to the `op` as the unfiltering occurs.
///
/// Each call to the `op` gets `|x, y, data|` as arguments, where `x` and `y`
/// are the position of the pixel data (relative to the top left), and `data` is
/// a slice of bytes representing the unfiltered pixel value at that location.
/// Bit-packed pixel data will be unpacked and have the callback called once per
/// pixel, with the data in the lowest bits of a single byte.
///
/// The data is unfiltered in place, and also each filter byte is reset to the
/// "no filter" setting as well. Thus, it's perfectly fine to call this more
/// than once on the same decompressed data if you just want to iterate the data
/// a second time for some reason.
///
/// ## Failure
/// * You **are** allowed to pass a `decompressed` buffer larger than just the
///   decompressed data itself. The function will use only the correct number of
///   bytes from the start of the buffer.
/// * If you for some reasons give a decompressed data buffer that is too small
///   then you'll get an error (possibly after some amount of the unfiltering is
///   done).
pub fn unfilter_decompressed_data<F>(
  header: IHDR, mut decompressed: &mut [u8], mut op: F,
) -> Result<(), PngError>
where
  F: FnMut(u32, u32, &[u8]),
{
  if header.width == 0 || header.height == 0 {
    return Err(PngError::ImageDimensionsTooSmall);
  }

  let filter_chunk_size = header.pixel_format.filter_chunk_size();

  // When the data is interlaced, we want to process the 1st through 7th reduced
  // images, so we take all of the image dimensions but drop the 0th one from
  // the iterator before we begin to use it. When the data is not interlaced we
  // take only the 0th image of the iterator (the full image).
  let mut image_it = reduced_image_dimensions(header.width, header.height)
    .into_iter()
    .enumerate()
    .map(|(i, (w, h))| (i, w, h))
    .take(if header.is_interlaced { 500 } else { 1 });
  if header.is_interlaced {
    image_it.next();
  }

  // From now on we're "always" working with reduced images because we've
  // re-stated the non-interlaced scenario as being a form of interlaced data,
  // which means we can stop thinking about the difference between if we're
  // interlaced or not.
  for (image_level, reduced_width, reduced_height) in image_it {
    if reduced_width == 0 || reduced_height == 0 {
      // while the full image's width and height must not be 0, the width or
      // height of any particular reduced image might still be 0.
      continue;
    }

    let bytes_per_filterline = header.pixel_format.bytes_per_scanline(reduced_width) + 1;
    let bytes_used_this_image = bytes_per_filterline.saturating_mul(reduced_height as _);

    let mut row_iter = if decompressed.len() < bytes_used_this_image {
      return Err(PngError::UnfilterWasNotGivenEnoughData);
    } else {
      let (these_bytes, more_bytes) = decompressed.split_at_mut(bytes_used_this_image);
      decompressed = more_bytes;
      these_bytes
        .chunks_exact_mut(bytes_per_filterline)
        .map(|chunk| {
          let (f, pixels) = chunk.split_at_mut(1);
          (&mut f[0], pixels)
        })
        .enumerate()
        .take(reduced_height as usize)
        .map(|(r_y, (f, pixels))| (r_y as u32, f, pixels))
    };

    // The first line of each image has special handling because filters can
    // refer to the previous line, but for the first line the "previous line" is
    // an implied zero.
    let mut b_pixels = if let Some((reduced_y, f, pixels)) = row_iter.next() {
      let mut p_it =
        pixels.chunks_exact_mut(filter_chunk_size).enumerate().map(|(r_x, d)| (r_x as u32, d));
      match f {
        1 => {
          // Sub
          let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some((reduced_x, pixel)) = p_it.next() {
            a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)| *p = p.wrapping_add(a));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        3 => {
          // Average
          let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some((reduced_x, pixel)) = p_it.next() {
            // the `b` is always 0, so we elide it from the computation
            a_pixel
              .iter()
              .copied()
              .zip(pixel.iter_mut())
              .for_each(|(a, p)| *p = p.wrapping_add(a / 2));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        4 => {
          // Paeth
          let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some((reduced_x, pixel)) = p_it.next() {
            // the `b` and `c` are both always 0
            a_pixel
              .iter()
              .copied()
              .zip(pixel.iter_mut())
              .for_each(|(a, p)| *p = p.wrapping_add(paeth_predict(a, 0, 0)));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        _ => {
          for (reduced_x, pixel) in p_it {
            // None and Up
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          }
        }
      }
      *f = 0;
      pixels
    } else {
      unreachable!("we already know that this image is at least 1 row");
    };

    for (reduced_y, f, pixels) in row_iter {
      let mut p_it =
        pixels.chunks_exact_mut(filter_chunk_size).enumerate().map(|(r_x, d)| (r_x as u32, d));
      let mut b_it = b_pixels.chunks_exact(filter_chunk_size);
      match f {
        1 => {
          // Sub
          let (reduced_x, mut pixel): (u32, &mut [u8]) = p_it.next().unwrap();
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some((reduced_x, pixel)) = p_it.next() {
            a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)| *p = p.wrapping_add(a));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        2 => {
          // Up
          for ((reduced_x, pixel), b_pixel) in p_it.zip(b_it) {
            b_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(b, p)| *p = p.wrapping_add(b));
            //
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          }
        }
        3 => {
          // Average
          let mut pb_it = p_it.zip(b_it).map(|((r_x, p), b)| (r_x, p, b));
          let (reduced_x, pixel, b_pixel) = pb_it.next().unwrap();
          pixel
            .iter_mut()
            .zip(b_pixel.iter().copied())
            .for_each(|(p, b)| *p = p.wrapping_add(b / 2));
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel: &[u8] = pixel;
          while let Some((reduced_x, pixel, b_pixel)) = pb_it.next() {
            a_pixel.iter().copied().zip(b_pixel.iter().copied()).zip(pixel.iter_mut()).for_each(
              |((a, b), p)| {
                *p = p.wrapping_add(((a as u32 + b as u32) / 2) as u8);
              },
            );
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        4 => {
          // Paeth
          let mut pb_it = p_it.zip(b_it).map(|((r_x, p), b)| (r_x, p, b));
          let (reduced_x, pixel, b_pixel) = pb_it.next().unwrap();
          pixel.iter_mut().zip(b_pixel.iter().copied()).for_each(|(p, b)| {
            *p = p.wrapping_add(paeth_predict(0, b, 0));
          });
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          let mut c_pixel = b_pixel;
          while let Some((reduced_x, pixel, b_pixel)) = pb_it.next() {
            a_pixel
              .iter()
              .copied()
              .zip(b_pixel.iter().copied())
              .zip(c_pixel.iter().copied())
              .zip(pixel.iter_mut())
              .for_each(|(((a, b), c), p)| {
                *p = p.wrapping_add(paeth_predict(a, b, c));
              });
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
            c_pixel = b_pixel;
          }
        }
        _ => {
          for (reduced_x, pixel) in p_it {
            // None
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          }
        }
      }
      b_pixels = pixels;
    }
  }

  //
  Ok(())
}
