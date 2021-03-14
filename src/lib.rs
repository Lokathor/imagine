#![no_std]
#![forbid(unsafe_code)]
#![allow(unused)]

use core::{
  convert::{TryFrom, TryInto},
  iter::Iterator,
};

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
  // TODO: read zlib header info

  let cur_slice = slices.next().unwrap();
  let more_slices = &mut slices;
  decompress_deflate_to(out, BitSource { cur_slice, more_slices })

  // TODO: read zlib adler32
}

struct BitSource<'b, I: Iterator<Item = &'b [u8]>> {
  cur_slice: &'b [u8],
  more_slices: I,
}

fn decompress_deflate_to<'b, I: Iterator<Item = &'b [u8]>>(out: &mut [u8], bit_src: BitSource<'b, I>) -> Result<(), ()> {
  Err(())
}
