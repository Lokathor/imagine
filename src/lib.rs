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

mod header;
pub use header::*;

pub fn decompress_idat_to(out: &mut [u8], png_bytes: &[u8]) -> Result<(), ()> {
  decompress_zlib_to(out, PngChunkIter::from_png_bytes(png_bytes).ok_or(())?.filter(|c| c.chunk_type == ChunkType::IDAT).map(|c| c.chunk_data))
}

fn decompress_zlib_to<'b>(out: &mut [u8], mut slices: impl Iterator<Item = &'b [u8]>) -> Result<(), ()> {
  let mut cur_slice = slices.next().ok_or(())?;
  trace!("decompress_zlib_to> {:?}", cur_slice);
  //
  let (cmf, rest) = cur_slice.split_first().ok_or(())?;
  cur_slice = rest;
  let cm = cmf & 0b1111;
  let cinfo = cmf >> 4;
  trace!("CMF: |{cmf:08b}| cm: {cm}, cinfo: {cinfo}", cmf = cmf, cm = cm, cinfo = cinfo);
  if cm != 8 {
    return Err(());
  }
  if cinfo > 7 {
    return Err(());
  }
  //
  let (flg, rest) = cur_slice.split_first().ok_or(())?;
  cur_slice = rest;
  let fcheck = 0b11111 & flg;
  let fdict = ((1 << 5) & flg) > 0;
  let flevel = flg >> 6;
  trace!("FLG: |{flg:08b}| fcheck: {fcheck}, fdict: {fdict}, flevel: {flevel}", flg = flg, fcheck = fcheck, fdict = fdict, flevel = flevel);
  let fcheck_pass = u16::from_be_bytes([*cmf, *flg]) % 31 == 0;
  trace!("fcheck is correct: {}", fcheck_pass);
  if !fcheck_pass {
    return Err(());
  }
  if fdict {
    return Err(());
  }

  let more_slices = &mut slices;
  decompress_deflate_to(out, BitSource { cur_slice, more_slices })

  // TODO: read zlib adler32
}

struct BitSource<'b, I: Iterator<Item = &'b [u8]>> {
  cur_slice: &'b [u8],
  more_slices: I,
}

fn decompress_deflate_to<'b, I: Iterator<Item = &'b [u8]>>(out: &mut [u8], bit_src: BitSource<'b, I>) -> Result<(), ()> {
  trace!("decompress_deflate_to> {}", {
    let mut s = String::new();
    for b in bit_src.cur_slice.iter().copied().rev() {
      s.push('|');
      s.push_str(&format!("{:08b}", b));
    }
    s.push_str("| <-- ");
    s
  });
  Err(())
}
