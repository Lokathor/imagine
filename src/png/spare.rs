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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PngChunkTy([u8; 4]);
impl PngChunkTy {
  pub const IHDR: Self = Self(*b"IHDR");
  pub const IDAT: Self = Self(*b"IDAT");
}
impl core::fmt::Debug for PngChunkTy {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Debug::fmt(core::str::from_utf8(self.0.as_slice()).unwrap_or("?"), f)
  }
}

#[derive(Debug, Clone, Copy)]
pub struct PngChunk<'b> {
  ty: PngChunkTy,
  data: &'b [u8],
  declared_crc: u32,
}
impl<'b> PngChunk<'b> {
  #[inline]
  #[must_use]
  pub const fn ty(&self) -> PngChunkTy {
    self.ty
  }
  #[inline]
  #[must_use]
  pub const fn data(&self) -> &'b [u8] {
    self.data
  }
  #[inline]
  #[must_use]
  pub const fn delcared_crc(&self) -> u32 {
    self.declared_crc
  }
  #[inline]
  #[must_use]
  pub fn compute_actual_crc(&self) -> u32 {
    let mut c = u32::MAX;
    self.ty.0.iter().copied().chain(self.data.iter().copied()).for_each(|b| {
      c = CRC_TABLE[((c ^ (b as u32)) & 0xFF) as usize] ^ (c >> 8);
    });
    c ^ u32::MAX
  }
}

pub struct PngChunkIter<'b> {
  spare: &'b [u8],
}
impl<'b> From<&'b [u8]> for PngChunkIter<'b> {
  #[inline]
  #[must_use]
  fn from(spare: &'b [u8]) -> Self {
    Self { spare }
  }
}
impl<'b> Iterator for PngChunkIter<'b> {
  type Item = PngChunk<'b>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      return None;
    }
    let (len, rest) = if self.spare.len() < 4 {
      self.spare = &[];
      return None;
    } else {
      let (len_bytes, rest) = self.spare.split_at(4);
      let len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
      (len, rest)
    };
    let (ty, rest) = if rest.len() < 4 {
      self.spare = &[];
      return None;
    } else {
      let (ty_bytes, rest) = rest.split_at(4);
      (PngChunkTy(ty_bytes.try_into().unwrap()), rest)
    };
    let (data, rest) = if rest.len() < len {
      self.spare = &[];
      return None;
    } else {
      rest.split_at(len)
    };
    let (declared_crc, rest) = if rest.len() < 4 {
      self.spare = &[];
      return None;
    } else {
      let (decl_crc_bytes, rest) = rest.split_at(4);
      (u32::from_be_bytes(decl_crc_bytes.try_into().unwrap()), rest)
    };
    self.spare = rest;
    Some(PngChunk { ty, data, declared_crc })
  }
}

const CRC_TABLE: [u32; 256] = {
  let mut table = [0_u32; 256];
  let mut n = 0;
  while n < 256 {
    let mut c: u32 = n as _;
    let mut k = 0;
    while k < 8 {
      if (c & 1) != 0 {
        c = 0xedb88320 ^ (c >> 1);
      } else {
        c = c >> 1;
      }
      //
      k += 1;
    }
    table[n] = c;
    //
    n += 1;
  }
  table
};

pub fn decode_png_to_image_rgba8(png: &[u8]) -> PngResult<ImageRGBA8> {
  let header = get_png_header(png)?;
  if header.width > 16384 || header.height > 16384 {
    return Err(PngError::ImageTooLargeForAutomaticDecoding);
  }
  let temp_mem_req = header.get_temp_memory_requirements()?;
  let it = get_png_idat(png)?;
  // It sucks that the standard library doesn't provide a way to just
  // try_allocate a zeroed byte vec in one step, but whatever.
  let mut temp_mem: Vec<u8> = Vec::new();
  temp_mem.try_reserve(temp_mem_req).map_err(|_| PngError::AllocationFailed)?;
  decompress_idat_to_temp_storage(&mut temp_mem, it)?;
  let final_pixel_count = (header.width * header.height) as usize;
  let mut final_mem: Vec<RGBA8> = Vec::new();
  final_mem.try_reserve(final_pixel_count).map_err(|_| PngError::AllocationFailed)?;
  let fline_len = header.get_temp_memory_bytes_per_filterline()?;
  // TODO: get palette data
  // TODO: get transparency data
  if header.interlace_method == PngInterlaceMethod::NO_INTERLACE {
    match (header.color_type, header.bit_depth) {
      (PngColorType::Y, 1) => {
        let line_iter = temp_mem.chunks_exact_mut(fline_len).map(|line| {
          let (f, bytes) = line.split_at_mut(1);
          (f[0], bytemuck::cast_slice_mut::<u8, [u8; 1]>(bytes))
        });
        unfilter_image(line_iter, |[y8]| {
          let mut i = 0b10000000;
          while i > 0 {
            let rgba8 = if y8 & i != 0 { [255, 255, 255, 255] } else { [0, 0, 0, 0] };
            final_mem.push(rgba8);
            i >>= 1;
          }
        })
      }
      (PngColorType::Y, 2) => {
        let line_iter = temp_mem.chunks_exact_mut(fline_len).map(|line| {
          let (f, bytes) = line.split_at_mut(1);
          (f[0], bytemuck::cast_slice_mut::<u8, [u8; 1]>(bytes))
        });
        unfilter_image(line_iter, |[y4]| {
          let mut i = 0b11000000;
          while i > 0 {
            let y = (y4 & i) * 0b01010101;
            let rgba8 = [y, y, y, 255];
            final_mem.push(rgba8);
            i >>= 1;
          }
        })
      }
      (PngColorType::Y, 4) => todo!(),
      (PngColorType::Y, 8) => todo!(),
      (PngColorType::Y, 16) => todo!(),
      (PngColorType::RGB, 8) => todo!(),
      (PngColorType::RGB, 16) => todo!(),
      (PngColorType::INDEX, 1) => todo!(),
      (PngColorType::INDEX, 2) => todo!(),
      (PngColorType::INDEX, 4) => todo!(),
      (PngColorType::INDEX, 8) => todo!(),
      (PngColorType::YA, 8) => todo!(),
      (PngColorType::YA, 16) => todo!(),
      (PngColorType::RGBA, 8) => todo!(),
      (PngColorType::RGBA, 16) => todo!(),
      _ => return Err(PngError::IllegalColorTypeBitDepthCombination),
    };
  } else {
    return Err(PngError::InterlaceNotSupported);
  }
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

#[derive(Debug, Clone, Copy)]
pub struct PngHeader {
  pub width: u32,
  pub height: u32,
  pub bit_depth: u8,
  pub color_type: PngColorType,
  pub compression_method: PngCompressionMethod,
  pub filter_method: PngFilterMethod,
  pub interlace_method: PngInterlaceMethod,
}
impl PngHeader {
  pub fn from_ihdr_chunk(chunk: PngChunk<'_>) -> PngResult<Self> {
    if chunk.ty() != PngChunkTy::IHDR || chunk.data().len() != 13 {
      Err(PngError::NotAnIhdrChunk)
    } else {
      let data = chunk.data();
      let width = u32::from_be_bytes(data[0..4].try_into().unwrap());
      let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
      let bit_depth = data[8];
      let color_type = PngColorType(data[9]);
      let compression_method = PngCompressionMethod(data[10]);
      let filter_method = PngFilterMethod(data[11]);
      let interlace_method = PngInterlaceMethod(data[12]);
      Ok(Self {
        width,
        height,
        bit_depth,
        color_type,
        compression_method,
        filter_method,
        interlace_method,
      })
    }
  }

  /// Returns the number of bytes per unfiltering chunk.
  ///
  /// Depending on bit depth and channel count, this can be from 1 to 8.
  pub fn get_filtered_pixel_size(self) -> PngResult<usize> {
    Ok(match self.color_type {
      PngColorType::Y if [1, 2, 4, 8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          2
        } else {
          1
        }
      }
      PngColorType::INDEX if [1, 2, 4, 8].contains(&self.bit_depth) => 1,
      PngColorType::RGB if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          6
        } else {
          3
        }
      }
      PngColorType::YA if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          4
        } else {
          2
        }
      }
      PngColorType::RGBA if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          8
        } else {
          4
        }
      }
      _ => return Err(PngError::IllegalColorTypeBitDepthCombination),
    })
  }

  /// The number of bytes in each filtered line of data.
  ///
  /// This is the bytes per scanline, +1 byte for the filter type.
  pub fn get_temp_memory_bytes_per_filterline(self) -> PngResult<usize> {
    if self.interlace_method == PngInterlaceMethod::NO_INTERLACE {
      let w = self.width as usize;
      if w == 0 {
        Err(PngError::IllegalWidthZero)
      } else {
        Ok(
          1 + match self.color_type {
            PngColorType::Y if [1, 2, 4, 8, 16].contains(&self.bit_depth) => {
              let bits_per_scanline =
                w.checked_mul(self.bit_depth as usize).ok_or(PngError::OutputOverflow)?;
              (bits_per_scanline + 7) / 8
            }
            PngColorType::INDEX if [1, 2, 4, 8].contains(&self.bit_depth) => {
              let bits_per_scanline =
                w.checked_mul(self.bit_depth as usize).ok_or(PngError::OutputOverflow)?;
              (bits_per_scanline + 7) / 8
            }
            PngColorType::RGB if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(3 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            PngColorType::YA if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(2 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            PngColorType::RGBA if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(4 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            _ => return Err(PngError::IllegalColorTypeBitDepthCombination),
          },
        )
      }
    } else {
      Err(PngError::InterlaceNotSupported)
    }
  }

  pub fn get_temp_memory_requirements(self) -> PngResult<usize> {
    if self.interlace_method == PngInterlaceMethod::NO_INTERLACE {
      let bytes_per_scanline: usize = self.get_temp_memory_bytes_per_filterline()?;
      let h = self.height as usize;
      if h == 0 {
        Err(PngError::IllegalHeightZero)
      } else {
        Ok(bytes_per_scanline * h)
      }
    } else {
      Err(PngError::InterlaceNotSupported)
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngColorType(u8);
impl PngColorType {
  pub const Y: Self = Self(0);
  pub const RGB: Self = Self(2);
  pub const INDEX: Self = Self(3);
  pub const YA: Self = Self(4);
  pub const RGBA: Self = Self(6);
}
impl core::fmt::Debug for PngColorType {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngColorType::Y => write!(f, "Y"),
      PngColorType::RGB => write!(f, "RGB"),
      PngColorType::INDEX => write!(f, "Index"),
      PngColorType::YA => write!(f, "YA"),
      PngColorType::RGBA => write!(f, "RGBA"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngCompressionMethod(u8);
impl PngCompressionMethod {
  pub const DEFLATE: Self = Self(0);
}
impl core::fmt::Debug for PngCompressionMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngCompressionMethod::DEFLATE => write!(f, "Deflate"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngFilterMethod(u8);
impl PngFilterMethod {
  pub const ADAPTIVE: Self = Self(0);
}
impl core::fmt::Debug for PngFilterMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngFilterMethod::ADAPTIVE => write!(f, "Adaptive"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngInterlaceMethod(u8);
impl PngInterlaceMethod {
  pub const NO_INTERLACE: Self = Self(0);
  pub const ADAM7: Self = Self(1);
}
impl core::fmt::Debug for PngInterlaceMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngInterlaceMethod::NO_INTERLACE => write!(f, "NoInterlace"),
      PngInterlaceMethod::ADAM7 => write!(f, "Adam7"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}
