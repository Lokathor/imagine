#![allow(non_camel_case_types)]

//! Holds all the tools for decoding PNG data.
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
//! # let mut idat_peek = RawPngChunkIter::new(&[]).map(PngChunk::try_from).peekable();
//! let idat_slice_it = idat_peek.filter_map(|r_chunk| match r_chunk {
//!   Ok(PngChunk::IDAT(IDAT { data })) => Some(data),
//!   _ => None,
//! });
//! decompress_idat_to_temp_storage(&mut temp_memory_buffer, idat_slice_it)?;
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
}
impl PngError {
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

/// Given an iterator over the filtered data of an image, unfilters all of the
/// data in place.
///
/// As each pixel is unfiltered it's also passed back out to the caller via
/// `op`. This allows you to place the pixel into its final memory while the
/// unfiltering is happening, instead of traversing the memory twice.
///
/// Filtering and Unfiltering are byte-wise operations on the pixels, so the
/// exact channel layout of each pixel does not matter. Only the bytes-per-pixel
/// (`BPP`) needs to be correct for unfiltering to take place.
///
/// Some channel and bit depth combinations will use less than 1 byte per pixel.
/// In this case, you should still use a `BPP` of 1, and each time `op` is
/// called you'll get a single byte of output that contains 2, 4, or 8 pixels
/// worth of output data (depending on pixel format). Note that if the number of
/// pixels in a line isn't an even multiple of the number of packed pixels per
/// byte then the last byte passed to `op` for a given line will have additional
/// zeroed bits on the end. This must be tracked by the caller.
///
/// The function assumes that all lines in the iterator will be the same length.
/// This is trivially true for non-interlaced images, but for interlaced images
/// you'll have to call the function once for each reduced image. However, the
/// `op` to place the data for each reduced image into the final memory is
/// already different for each reduced image, so in practice you already had to
/// call this once for each reduced image.
pub fn unfilter_image<'b, I, F, const BPP: usize>(mut line_iter: I, mut op: F)
where
  I: Iterator<Item = (&'b mut u8, &'b mut [[u8; BPP]])>,
  F: FnMut([u8; BPP]),
{
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
  //
  let mut b_line = if let Some((f, x_line)) = line_iter.next() {
    match f {
      1 => {
        // "sub"
        let mut x_line_iter = x_line.iter_mut();
        let mut a = if let Some(a) = x_line_iter.next() { a } else { return };
        while let Some(x) = x_line_iter.next() {
          for (x_byte, a_byte) in x.iter_mut().zip(a.iter()) {
            *x_byte = x_byte.wrapping_add(*a_byte);
          }
          op(*x);
          a = x;
        }
      }
      2 => (/* Up filter has no effect on the first line */),
      3 => {
        // "average"
        let mut x_line_iter = x_line.iter_mut();
        let mut a = if let Some(a) = x_line_iter.next() { a } else { return };
        while let Some(x) = x_line_iter.next() {
          for (x_byte, a_byte) in x.iter_mut().zip(a.iter()) {
            *x_byte = x_byte.wrapping_add(a_byte >> 1);
          }
          op(*x);
          a = x;
        }
      }
      4 => {
        // "paeth"
        let mut x_line_iter = x_line.iter_mut();
        let mut a = if let Some(a) = x_line_iter.next() { a } else { return };
        while let Some(x) = x_line_iter.next() {
          for (x_byte, a_byte) in x.iter_mut().zip(a.iter()) {
            *x_byte = x_byte.wrapping_add(paeth_predict(*a_byte, 0, 0));
          }
          op(*x);
          a = x;
        }
      }
      _ => (),
    }
    *f = 0;
    x_line
  } else {
    return;
  };
  //
  while let Some((f, x_line)) = line_iter.next() {
    match f {
      1 => {
        // "sub"
        let mut x_line_iter = x_line.iter_mut();
        let mut a = if let Some(a) = x_line_iter.next() { a } else { return };
        while let Some(x) = x_line_iter.next() {
          for (x_byte, a_byte) in x.iter_mut().zip(a.iter()) {
            *x_byte = x_byte.wrapping_add(*a_byte);
          }
          op(*x);
          a = x;
        }
      }
      2 => {
        for (x, b) in x_line.iter_mut().zip(b_line.iter()) {
          for (x_byte, b_byte) in x.iter_mut().zip(b.iter()) {
            *x_byte = x_byte.wrapping_add(*b_byte);
          }
        }
      }
      3 => {
        // "average"
        let mut xb_line_iter = x_line.iter_mut().zip(b_line.iter());
        let mut a = if let Some((x, b)) = xb_line_iter.next() {
          for (x_byte, b_byte) in x.iter_mut().zip(b.iter()) {
            *x_byte = x_byte.wrapping_add(b_byte >> 1);
          }
          x
        } else {
          return;
        };
        while let Some((x, b)) = xb_line_iter.next() {
          for ((x_byte, a_byte), b_byte) in x.iter_mut().zip(a.iter()).zip(b.iter()) {
            *x_byte = x_byte.wrapping_add(((*a_byte as u32 + *b_byte as u32) >> 1) as u8);
          }
          op(*x);
          a = x;
        }
      }
      4 => {
        // "paeth"
        let mut xb_line_iter = x_line.iter_mut().zip(b_line.iter());
        let (mut a, mut c) = if let Some((x, b)) = xb_line_iter.next() {
          for (x_byte, b_byte) in x.iter_mut().zip(b.iter()) {
            *x_byte = x_byte.wrapping_add(paeth_predict(0, *b_byte, 0));
          }
          (x, b)
        } else {
          return;
        };
        while let Some((x, b)) = xb_line_iter.next() {
          for (((x_byte, a_byte), b_byte), c_byte) in
            x.iter_mut().zip(a.iter()).zip(b.iter()).zip(c.iter())
          {
            *x_byte = x_byte.wrapping_add(paeth_predict(*a_byte, *b_byte, *c_byte));
          }
          op(*x);
          a = x;
          c = b;
        }
      }
      _ => (),
    }
    *f = 0;
    //
    b_line = x_line;
  }
}
