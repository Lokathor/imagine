#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]

//! Holds all the tools for decoding PNG data.
//!
//! The `png` portion of the crate uses no `unsafe` code. Further, it should not
//! even be possible to make the library panic. However, bugs do occur, and
//! hostile PNG files might be able to make the library panic. Please [file an
//! issue](https://github.com/Lokathor/imagine) if this does occur.
//!
//! ## Automated PNG Decoding
//! If you don't need full control over the decoding process there's functions
//! provided that will take a PNG data stream and just give you the pixels in a
//! `Vec<RGBA8>` (along with other info like width, height, etc).
//!
//! These functions do the allocation for you, and so they require that the
//! `alloc` feature be enabled.
//!
//! ## Decoding a PNG Yourself
//! This crate allows you to directly control the entire PNG decoding process
//! yourself if you wish. The advantage of doing this is that you can avoid any
//! excess allocation.
//!
//! The general format of a PNG is that the information is stored in "chunks".
//! You iterate the chunks and each one gives you some info that you might
//! decide to use. There's four "critical" chunk types:
//! * **Header** - This has all the important information about the image's
//!   dimensions, pixel format, and if the image is interlaced or not. Using
//!   this information you'll be able to know how much temporary space is
//!   required for decompression, and how much final space is required after
//!   unfiltering.
//! * **Palette** - If an image uses indexed color it will have a palette of
//!   what index values map to what `RGB8` values.
//! * **Image Data** - One or more chunks of compressed data. All of the
//!   compressed data forms a single zlib data stream. All of the image data
//!   chunks should appear one after the other.
//! * **End** - The last chunk, lets you know you had the full PNG and your data
//!   wasn't truncated accidentally.
//!
//! After the header and before the image data there are also zero or more
//! "ancillary" chunks which might give you additional information about the
//! image. If you just want to display the image, the ancillary chunk that's
//! most likely to be important to you is if there's a transparency chunk.
//!
//! ### Step By Step
//!
//! First you'll want an iterator over the PNG chunks. In this example, we use a
//! raw chunk iterator, parse each raw chunk into a more structured chunk value,
//! and then filter any errors for only the most critical errors using the
//! [`critical_errors_only`] helper filter.
//!
//! ```no_run
//! use imagine::png::*;
//! let png: &[u8] = unimplemented!("data from somewhere");
//! let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);
//! ```
//!
//! Once the iterator is ready you need to get the header data. This comes in
//! the form of an [`IHDR`] chunk, and it should be the very first chunk you
//! find. Assuming that you're inside of a function that returns `Result<_,
//! PngError>` you'd use a few `?` operators here and there, something like
//! this.
//!
//! ```no_run
//! # use imagine::png::*;
//! # fn or_png_error(png: &[u8]) -> Result<(), PngError> {
//! #  let mut it = RawPngChunkIter::new(&[]).map(PngChunk::try_from);
//! let ihdr: IHDR =
//!   it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;
//! # Ok(())
//! # }
//! ```
//!
//! Now you have the PNG's header information. This tells you:
//! * The dimensions of the image
//! * The pixel format of the image's pixels
//! * If the decompressed data of the image is interlaced or not.
//!
//! When storing the PNG, the raw pixel values are first "filtered" (to try and
//! make them more compression-friendly), and then compressed into a Zlib data
//! stream. To decode the PNG info you have to reverse the operations. First
//! decompressing, and then unfiltering.
//!
//! * **Unfiltering:** The decompressed data will be a series of lines for
//!   images with an extra byte on the front of each line which says what filter
//!   method was used for that line. The unfiltering can be performed in place,
//!   but leaves the filter markers between each line of useful pixel data. Most
//!   other code doesn't expect this layout, so you'll usually have to copy the
//!   lines into a different buffer.
//! * **De-interlacing:** If the image is interlaced then the data won't be
//!   stored as a single series of filtered bytes and lines. Instead, the
//!   overall image is stored as a series seven "reduced" images of varying
//!   resolutions. Again, most code doesn't expect this arrangement of the data,
//!   so you'll usually have to de-interlace the data to make it usable.
//! * **Flipping:** The PNG format assumes that the origin pixel is the top
//!   left, with scanlines going left to right, top to bottom. If your use of
//!   the data doesn't have this same assumption you'll need to flip the rows
//!   and/or columns of the pixels.
//! * **Pixel Format Changes:** The PNG's stored pixel format might not be the
//!   same as your desired target pixel format. Particularly, any pixel format
//!   that packs multiple pixels within a byte is unlikely to be usable by
//!   common code.
//!
//! The decompression is generally done as its own stage of work.
//!
//! All the other steps (unfiltering, de-interlacing, etc) can generally be
//! combined into just one additional pass over the decompressed data that
//! unfilters the data in place while also optionally passing out info to a
//! callback which get's the PNG position and the data, and can then perform any
//! position or format changes.
//!
//! With all of that explanation out of the way, we can get our buffer to
//! decompress the zlib stream into. For this we use the
//! [temp_memory_requirement](IHDR::temp_memory_requirement) method of our
//! header. This will return the number of bytes that we'll need. You could
//! allocate a new buffer, or perhaps you're decoding many PNGs and you already
//! have a sufficiently large buffer from the last PNG, either way is fine.
//!
//! ```no_run
//! # use imagine::png::*;
//! # let ihdr = IHDR {width: 0, height: 0, pixel_format: PngPixelFormat::Y1, is_interlaced: false};
//! let mut temp_memory_buffer: Vec<u8> = vec![0; ihdr.temp_memory_requirement()];
//! ```
//!
//! And then we can decompress the `IDAT` chunk data into the temporary buffer.
//! A function for this is provided called [decompress_idat_to_temp_storage],
//! which does basically what it says. To make our chunk iterator from before
//! first we want to advance the iterator so that the `IDAT` is the next chunk
//! available. For this we'll use the [peekable](Iterator::peekable) method,
//! then we'll keep peeking at the next output until the `IDAT` would be the
//! next output.
//!
//! ```no_run
//! # use imagine::png::*;
//! # fn or_png_error(png: &[u8]) -> Result<(), PngError> {
//! # let mut it = RawPngChunkIter::new(&[]).map(PngChunk::try_from);
//! let mut idat_peek = it.peekable();
//! loop {
//!   match idat_peek.peek() {
//!     Some(Ok(PngChunk::IDAT(_))) => break,
//!     None => return Err(PngError::NoIDATChunks),
//!     _ => {
//!       idat_peek.next();
//!     }
//!   }
//! }
//! # unimplemented!();
//! # }
//! ```
//!
//! Now that the `idat_peek` iterator is in position, we'll convert it to a
//! slice iterator using [filter_map](Iterator::filter_map) method, and then run
//! the decompression.
//!
//! ```no_run
//! # use imagine::png::*;
//! # fn or_png_error(png: &[u8]) -> Result<(), PngError> {
//! # let mut idat_peek = RawPngChunkIter::new(&[]).map(PngChunk::try_from).peekable();
//! # let mut temp_memory_buffer: Vec<u8> = vec![0; 0];
//! let idat_slice_it = idat_peek.filter_map(|r_chunk| match r_chunk {
//!   Ok(PngChunk::IDAT(IDAT { data })) => Some(data),
//!   _ => None,
//! });
//! decompress_idat_to_temp_storage(&mut temp_memory_buffer, idat_slice_it)?;
//! # unimplemented!();
//! # }
//! ```
//!
//! This gives us the filtered bytes in the output buffer. Now we still have to
//! unfilter the data.
//!
//! Each scanline of the image has a filter byte which says what filter applies
//! to the rest of that scanline. There are four filters that have an effect, as
//! well as a "no op" filter that doesn't actually change the data. Each
//! scanline can use a separate filter type, based on what the PNG encoder
//! thought was best. The filtered and unfiltered data take up the same amount
//! of space, so the unfiltering is performed "in place" on the temporary
//! buffer's bytes. As individual pixels are produced by the unfiltering they're
//! both written back to the temporary buffer (because they can affect future
//! scanlines), and they're also passed to a user-provided callback which gets
//! the position within the PNG as well as the pixel data.
//! * The callback can flip the pixel positions when writing out the data if a
//!   horizontal or vertical flip is required.
//! * Any interlaced images will give pixels to the callback out of order, but
//!   each pixel should appear just once, so as long as your callback writes to
//!   the correct position each time the image will be automatically
//!   de-interlaced as it's unfiltered.
//!
//! The filtering process works on byte arrays at a time, regardless of the
//! exact format of a pixel. For example, the `Y16` and `YA8` formats are both 2
//! bytes each, and so will unfilter `[u8; 2]` at a time, regardless of the
//! channel differences. The [`cast`](bytemuck::cast) function is your friend
//! here.
//!
//! One complication is that if there's less than 8 bits per pixel then there
//! will be more than one pixel packed within a single byte. This can happen
//! with both grayscale and indexed images. In this situation your callback will
//! be run once for each individual pixel, and each call will get a single byte
//! with the correct data in the *lowest* bits of each output byte.
//!
//! **TODO:** how do sub-8 bit depths and interlacing interact

mod chunks;
pub use chunks::*;
use miniz_oxide::inflate::{
  core::{
    decompress,
    inflate_flags::{
      TINFL_FLAG_HAS_MORE_INPUT, TINFL_FLAG_IGNORE_ADLER32, TINFL_FLAG_PARSE_ZLIB_HEADER,
      TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    },
    DecompressorOxide,
  },
  TINFLStatus,
};

/// The first eight bytes of a PNG datastream should match these bytes.
pub const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Errors that can happen when trying to process a PNG.
///
/// Many of these don't actually prevent all progress with parsing. Usually only
/// a particular chunk is unusable, and you can just ignore that chunk and
/// proceed. The [`is_critical`](PngError::is_critical) method is a quick way to
/// separate the critical errors from non-critical errors.
///
/// Many errors are just "Illegal_Foo", for various chunk types Foo. The precise
/// details of what's wrong inside of a chunk's data aren't usually that
/// interesting. If you want more fine grained results in this area I'm happy to
/// accept a PR about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum PngError {
  /// If the header isn't the first chunk you're pretty boned because you don't
  /// know the image's dimensions or pixel format.
  FirstChunkNotIHDR,
  Illegal_IHDR,
  Illegal_PLTE,
  /// Though the `IEND` is a "critical chunk", this is not considered a critical
  /// error, because you've already processed all the data at this point.
  Illegal_IEND,
  Illegal_cHRM,
  Illegal_gAMA,
  Illegal_iCCP,
  Illegal_sBIT,
  Illegal_sRGB,
  Illegal_tEXt,
  Illegal_zTXt,
  Illegal_iTXt,
  Illegal_bKGD,
  Illegal_hIST,
  Illegal_pHYs,
  Illegal_sPLT,
  Illegal_tIME,
  UnknownChunkType,
  NoChunksPresent,
  NoIDATChunks,
  DecompressionFailure,
  DecompressionOverflow,
  UnfilterWasNotGivenEnoughData,
  ImageDimensionsTooSmall,
}
impl PngError {
  /// Returns `true` if the error is a critical chunk parsing error.
  pub fn is_critical(self) -> bool {
    use PngError::*;
    match self {
      FirstChunkNotIHDR | Illegal_IHDR | Illegal_PLTE => true,
      _ => false,
    }
  }
}

/// Useful as a [`filter`](Iterator::filter) over chunk parsing results so that
/// non-critical errors are filtered away.
pub fn critical_errors_only(r: &Result<PngChunk, PngError>) -> bool {
  match r {
    Ok(_) => true,
    Err(e) if e.is_critical() => true,
    _ => false,
  }
}

/// Decompresses IDAT bytes to the temporary buffer.
///
/// This doesn't give you the final bytes. This gives you the filtered bytes.
/// The filtered bytes must then be unfiltered to get the final values.
pub fn decompress_idat_to_temp_storage<'out, 'inp>(
  out: &'out mut [u8], it: impl Iterator<Item = &'inp [u8]>,
) -> Result<(), PngError> {
  let mut it = it.peekable();
  let r = &mut DecompressorOxide::new();
  let mut out_pos = 0;
  let mut zlib_header = true;
  while let Some(in_buf) = it.next() {
    let has_more = it.peek().is_some();
    let flags = if zlib_header { TINFL_FLAG_PARSE_ZLIB_HEADER } else { 0 }
      | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
      | TINFL_FLAG_IGNORE_ADLER32
      | if has_more { TINFL_FLAG_HAS_MORE_INPUT } else { 0 };
    let (status, _input_read, bytes_written) = decompress(r, in_buf, out, out_pos, flags);
    zlib_header = false;
    out_pos += bytes_written;
    match status {
      TINFLStatus::Done | TINFLStatus::Adler32Mismatch => return Ok(()),
      TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
        if !has_more {
          return Err(PngError::DecompressionFailure);
        } else {
          continue;
        }
      }
      TINFLStatus::BadParam | TINFLStatus::Failed => return Err(PngError::DecompressionFailure),
      TINFLStatus::HasMoreOutput => return Err(PngError::DecompressionOverflow),
    }
  }
  Ok(())
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
  use core::iter::repeat;
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
    // expression.
    if pa <= pb && pa <= pc {
      a
    } else if pb <= pc {
      b
    } else {
      c
    }
  }
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
  //
  if header.width == 0 || header.height == 0 {
    return Err(PngError::ImageDimensionsTooSmall);
  }
  // When the data is interlaced, we want to process the 1st through 7th reduced
  // images, so we take all of the image dimensions but drop the 0th one from
  // the iterator before we begin to use it. When the data is not interlaced we
  // take only the 0th image of the iterator (the full image).
  let mut it = reduced_image_dimensions(header.width, header.height)
    .into_iter()
    .enumerate()
    .map(|(i, (w, h))| (i, w, h))
    .take(if header.is_interlaced { 500 } else { 1 });
  if header.is_interlaced {
    it.next();
  }

  // From now on we're "always" working with reduced images because we've
  // re-stated the non-interlaced scenario as being a form of interlaced data,
  // which means we can stop thinking about the difference between if we're
  // interlaced or not.
  for (image_level, reduced_width, reduced_height) in it {
    if reduced_width == 0 || reduced_height == 0 {
      // while the full image's width and height must not be 0, the width or
      // height of any particular reduced image might still be 0.
      continue;
    }
    let bytes_per_filterline = header.pixel_format.bytes_per_scanline(reduced_width) + 1;
    let bytes_used_this_image = bytes_per_filterline.saturating_mul(reduced_height as _);
    if decompressed.len() < bytes_used_this_image {
      return Err(PngError::UnfilterWasNotGivenEnoughData);
    }
    let mut filtered_image_iter = {
      decompressed
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
    //
    // A "cleaner" way might be to have a single iterator over the fake line and
    // also all real lines and then have it give us two lines at a time
    // (previous and current) and then we only have to write the content of that
    // loop once without special casing. However, I'm not entirely sure how to
    // write that iterator.
    let (reduced_y, f, pixels) = filtered_image_iter.next().unwrap();
    {
      let mut line_it = pixels
        .chunks_exact_mut(header.pixel_format.bytes_per_pixel())
        .enumerate()
        .map(|(r_x, d)| (r_x as u32, d));
      match f {
          1 /* Sub */ => {
            let (reduced_x, mut a_pixel): (u32, &mut [u8]) = line_it.next().unwrap();
            send_out_pixel(header, image_level, reduced_x, reduced_y, a_pixel, &mut op);
            while let Some((reduced_x, pixel)) = line_it.next() {
              a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)| *p = p.wrapping_add(a));
              send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          3 /* Average */ => {
            let (r_x, mut a_pixel): (u32, &mut [u8]) = line_it.next().unwrap();
            while let Some((reduced_x, pixel)) = line_it.next() {
              // the `b` is always 0, so we elide it from the computation
              a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)|
                *p = p.wrapping_add(a / 2)
              );
              send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          4 /* Paeth */ => {
            let (r_x, mut a_pixel): (u32, &mut [u8]) = line_it.next().unwrap();
            while let Some((reduced_x, pixel)) = line_it.next() {
              // the `b` and `c` are both always 0, so we elide them from the computation
              a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)|
                *p = p.wrapping_add(paeth_predict(a, 0, 0))
              );
              send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          },
          _ /* None and Up */ => for (reduced_x, pixel) in line_it {
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          },
        }
    }
    *f = 0;
    let mut b_pixels = pixels;

    for (reduced_y, f, pixels) in filtered_image_iter {
      let mut line_it = pixels
        .chunks_exact_mut(header.pixel_format.bytes_per_pixel())
        .enumerate()
        .map(|(r_x, d)| (r_x as u32, d));
      let mut b_it = b_pixels.chunks_exact(header.pixel_format.bytes_per_pixel());
      match f {
        1 /* Sub */ => {
          let (reduced_x, mut pixel): (u32, &mut [u8]) = line_it.next().unwrap();
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some((reduced_x, pixel)) = line_it.next() {
            a_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(a, p)| *p = p.wrapping_add(a));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        2 /* Up */ =>  {
          for ((reduced_x, pixel), b_pixel) in line_it.zip(b_it) {
            b_pixel.iter().copied().zip(pixel.iter_mut()).for_each(|(b, p)| *p = p.wrapping_add(b));
            //
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          }
        }
        3 /* Average */ =>  {
          let mut ab_it = line_it.zip(b_it);
          let ((reduced_x, mut pixel), b_pixel) = ab_it.next().unwrap();
          pixel.iter_mut().zip(b_pixel.iter().copied()).for_each(|(p, b)| *p = p.wrapping_add(b/2));
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          while let Some(((reduced_x, pixel), b_pixel)) = ab_it.next() {
            a_pixel
              .iter()
              .copied()
              .zip(b_pixel.iter().copied())
              .zip(pixel.iter_mut())
              .for_each(|((a,b), p)| *p = p.wrapping_add(((a as usize + b as usize)/2)as u8));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
          }
        }
        4 /* Paeth */ =>  {
          let mut ab_it = line_it.zip(b_it);
          let ((reduced_x, mut pixel), b_pixel) = ab_it.next().unwrap();
          pixel.iter_mut().zip(b_pixel.iter().copied()).for_each(|(p, b)| *p = p.wrapping_add(b/2));
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
          let mut a_pixel = pixel;
          let mut c_pixel = b_pixel;
          while let Some(((reduced_x, pixel), b_pixel)) = ab_it.next() {
            a_pixel
              .iter()
              .copied()
              .zip(b_pixel.iter().copied())
              .zip(pixel.iter_mut())
              .for_each(|((a,b), p)| *p = p.wrapping_add(((a as usize + b as usize)/2)as u8));
            send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
            //
            a_pixel = pixel;
            c_pixel = b_pixel;
          }
        }
        _ /* None */ => for (reduced_x, pixel) in line_it {
          send_out_pixel(header, image_level, reduced_x, reduced_y, pixel, &mut op);
        },
      }
      b_pixels = pixels;
    }
    decompressed = &mut decompressed[bytes_used_this_image..];
  }

  //
  Ok(())
}
