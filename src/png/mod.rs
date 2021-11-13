use alloc::alloc::{alloc_zeroed, Layout};

use self::chunk::{PngChunk, PngChunkTy};
use crate::{png::chunk::PngChunkIter, Image, ImageRGBA8, RGBA8};
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

mod chunk;
pub use chunk::*;

mod ihdr;
pub use ihdr::*;

pub fn decode_png_to_image_rgba8(png: &[u8]) -> PngResult<ImageRGBA8> {
  let header = get_png_header(png)?;
  if header.width > 16384 || header.height > 16384 {
    return Err(PngError::ImageTooLargeForAutomaticDecoding);
  }
  let temp_mem_req = header.get_temp_memory_requirements()?;
  let it = get_png_idat(png)?;
  // It sucks that the standard library doesn't provide a way to just
  // try_allocate a zeroed byte vec in one step, but whatever.
  let mut temp_mem: Vec<u8> = unsafe {
    let ptr: *mut u8 = alloc_zeroed(
      Layout::from_size_align(temp_mem_req, 1).map_err(|_| PngError::AllocationFailed)?,
    );
    if ptr.is_null() {
      return Err(PngError::AllocationFailed);
    }
    Vec::from_raw_parts(ptr, temp_mem_req, temp_mem_req)
  };
  decompress_idat_to_temp_storage(&mut temp_mem, it)?;
  let final_pixel_count = (header.width * header.height) as usize;
  let mut final_mem: Vec<RGBA8> = unsafe {
    let ptr: *mut RGBA8 = alloc_zeroed(
      Layout::from_size_align(final_pixel_count * 4, 1).map_err(|_| PngError::AllocationFailed)?,
    )
    .cast();
    if ptr.is_null() {
      return Err(PngError::AllocationFailed);
    }
    Vec::from_raw_parts(ptr, final_pixel_count, final_pixel_count)
  };
  todo!("unfilter the bytes");
  Ok(Image { width: header.width, height: header.height, pixels: final_mem })
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum PngError {
  BytesAreNotPng,
  NoChunksDetected,
  IdatNotFound,
  IdatDecompressionFailed,
  IdatOutputOverflow,
  IhdrIllegalData,
  InterlaceNotSupported,
  /// The `decode_png_to_image` function will abort the attempt if the image is
  /// more than 16,384 pixels in either `width` or `height`.
  ///
  /// Only `decode_png_to_image`, which allocates memory on its own, will give
  /// this error. If you unpack the image yourself then you can bypass this
  /// issue.
  ImageTooLargeForAutomaticDecoding,
  AllocationFailed,
  NotAnIhdrChunk,
  IllegalColorTypeBitDepthCombination,
  IllegalWidthZero,
  IllegalHeightZero,
  OutputOverflow,
  FilteredBytesLengthMismatch,
  OutBufferLengthMismatch,
}
pub type PngResult<T> = Result<T, PngError>;

pub fn get_png_header(png: &[u8]) -> PngResult<PngHeader> {
  const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
  //
  if png.len() < 8 || &png[..8] != PNG_SIGNATURE {
    return Err(PngError::BytesAreNotPng);
  } else if let Some(chunk) = PngChunkIter::from(&png[8..]).next() {
    PngHeader::from_ihdr_chunk(chunk)
  } else {
    Err(PngError::NoChunksDetected)
  }
}

pub fn get_png_idat(png: &[u8]) -> PngResult<impl Iterator<Item = &[u8]>> {
  if png.len() < 8 {
    return Err(PngError::BytesAreNotPng);
  }
  let mut it = PngChunkIter::from(&png[8..]).peekable();
  while let Some(chunk) = it.peek() {
    if chunk.ty() == PngChunkTy::IDAT {
      return Ok(it.map(|ch| ch.data()));
    } else {
      it.next();
    }
  }
  Err(PngError::IdatNotFound)
}

/// Decompresses IDAT bytes to the temporary buffer.
///
/// This doesn't give you the final bytes. This gives you the filtered bytes.
/// The filtered bytes must then be unfiltered to get the final values.
pub fn decompress_idat_to_temp_storage<'out, 'inp>(
  out: &'out mut [u8], it: impl Iterator<Item = &'inp [u8]>,
) -> PngResult<()> {
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
          return Err(PngError::IdatDecompressionFailed);
        } else {
          continue;
        }
      }
      TINFLStatus::BadParam | TINFLStatus::Failed => return Err(PngError::IdatDecompressionFailed),
      TINFLStatus::HasMoreOutput => return Err(PngError::IdatOutputOverflow),
    }
  }
  Ok(())
}

const fn paeth_predict(a: u8, b: u8, c: u8) -> u8 {
  let a_ = a as i32;
  let b_ = b as i32;
  let c_ = c as i32;
  let p: i32 = a_ + b_ - c_;
  let pa = (p - a_).abs();
  let pb = (p - b_).abs();
  let pc = (p - c_).abs();
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
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
  I: Iterator<Item = (u8, &'b mut [[u8; BPP]])>,
  F: FnMut([u8; BPP]),
{
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
    //
    b_line = x_line;
  }
}
