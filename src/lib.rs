//#![no_std]
#![forbid(unsafe_code)]
#![allow(unused)]

use core::{
  convert::{TryFrom, TryInto},
  iter::Iterator,
};

#[macro_export]
macro_rules! trace {
  ($($arg:tt)*) => {
    print!("{file}:{line}> ", file = file!(), line = line!());
    println!($($arg)*);
  }
}

mod chunk;
pub use chunk::*;

mod chunk_iter;
pub use chunk_iter::*;

mod png_header;
pub use png_header::*;

mod bit_source;
use bit_source::*;

pub type PngResult<T> = Result<T, PngError>;

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum PngError {
  UnexpectedEndOfInput,
  NoPngSignature,
  IllegalCompressionMethod,
  IllegalCompressionInfo,
  IllegalFlagCheck,
  IllegalFlagDictionary,
  IllegalBlockType,
}

pub fn decompress_idat_to(out: &mut [u8], png_bytes: &[u8]) -> PngResult<()> {
  decompress_zlib_to(
    out,
    PngChunkIter::from_png_bytes(png_bytes)?
      .filter(|c| c.chunk_type == ChunkType::IDAT)
      .map(|c| c.chunk_data),
  )
}

fn decompress_zlib_to<'b>(
  out: &mut [u8], mut slices: impl Iterator<Item = &'b [u8]>,
) -> PngResult<()> {
  let mut cur_slice = slices.next().ok_or(PngError::UnexpectedEndOfInput)?;
  trace!("decompress_zlib_to> {:?}", cur_slice);
  //
  let (cmf, rest) = cur_slice.split_first().ok_or(PngError::UnexpectedEndOfInput)?;
  cur_slice = rest;
  let cm = cmf & 0b1111;
  let cinfo = cmf >> 4;
  trace!("CMF: |{cmf:08b}| cm: {cm}, cinfo: {cinfo}", cmf = cmf, cm = cm, cinfo = cinfo);
  if cm != 8 {
    return Err(PngError::IllegalCompressionMethod);
  }
  if cinfo > 7 {
    return Err(PngError::IllegalCompressionInfo);
  }
  //
  let (flg, rest) = cur_slice.split_first().ok_or(PngError::UnexpectedEndOfInput)?;
  cur_slice = rest;
  let fcheck = 0b11111 & flg;
  let fdict = ((1 << 5) & flg) > 0;
  let flevel = flg >> 6;
  trace!(
    "FLG: |{flg:08b}| fcheck: {fcheck}, fdict: {fdict}, flevel: {flevel}",
    flg = flg,
    fcheck = fcheck,
    fdict = fdict,
    flevel = flevel
  );
  let fcheck_pass = u16::from_be_bytes([*cmf, *flg]) % 31 == 0;
  trace!("fcheck is correct: {}", fcheck_pass);
  if !fcheck_pass {
    return Err(PngError::IllegalFlagCheck);
  }
  if fdict {
    return Err(PngError::IllegalFlagCheck);
  }

  let mut bit_source = BitSource::new(cur_slice, slices);
  decompress_deflate_to(out, &mut bit_source)?;

  // TODO: read zlib adler32
  Err(PngError::UnexpectedEndOfInput)
}

fn decompress_deflate_to<'b, I: Iterator<Item = &'b [u8]>>(
  out: &mut [u8], bit_src: &mut BitSource<'b, I>,
) -> PngResult<()> {
  trace!("decompress_deflate_to> {:?}", bit_src);

  loop {
    trace!("Begin Block Loop");

    let is_final_block = bit_src.get_bfinal()?;
    trace!("{:?}", bit_src);
    trace!("is_final_block: {:?}", is_final_block);

    let block_type = bit_src.get_btype()?;
    trace!("{:?}", bit_src);
    trace!("block_type: {:02b}", block_type);
    debug_assert!(block_type < 0b100);

    match block_type {
      0 => (),
      1 => (),
      2 => (),
      3 => return Err(PngError::IllegalBlockType),
      _ => unimplemented!(),
    }

    if is_final_block {
      return Ok(());
    }
  }
}
