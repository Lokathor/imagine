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
  unfilter_bytes_to_rgba8(&mut final_mem, &temp_mem, header, png)?;
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

/// Unfilters bytes (of any pixel format) into RGBA8 pixels.
pub fn unfilter_bytes_to_rgba8<'out, 'inp>(
  out: &mut [RGBA8], filtered_bytes: &'inp [u8], header: PngHeader, png: &[u8],
) -> PngResult<()> {
  if header.interlace_method != PngInterlaceMethod::NO_INTERLACE {
    return Err(PngError::InterlaceNotSupported);
  }
  let bytes_per_scanline = header.get_temp_memory_bytes_per_scanline()?;
  if bytes_per_scanline.saturating_mul(header.height as usize) != filtered_bytes.len() {
    return Err(PngError::FilteredBytesLengthMismatch);
  }

  todo!()
}
