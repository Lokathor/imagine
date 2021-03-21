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

mod huff_symbol;
use huff_symbol::*;

mod tree_entry;
use tree_entry::*;

mod code_length_alphabet;
use code_length_alphabet::*;

mod lit_len_alphabet;
use lit_len_alphabet::*;

mod dist_alphabet;
use dist_alphabet::*;

mod fixed_huffman_tree;
use fixed_huffman_tree::*;

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
  CouldNotFindLitLenSymbol,
  CouldNotFindDistSymbol,
  OutputOverflow,
  BackRefToBeforeOutputStart,
  DidNotWriteAdler32Yet,
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
  Err(PngError::DidNotWriteAdler32Yet)
}

fn decompress_deflate_to<'b, I: Iterator<Item = &'b [u8]>>(
  out: &mut [u8], bit_src: &mut BitSource<'b, I>,
) -> PngResult<()> {
  trace!("decompress_deflate_to> {:?}", bit_src);

  let mut out_position = 0;

  'per_block: loop {
    trace!("Begin Block Loop");

    trace!("{:?}", bit_src);
    let is_final_block = bit_src.next_bfinal()?;
    trace!("is_final_block: {:?}", is_final_block);

    trace!("{:?}", bit_src);
    let block_type = bit_src.next_btype()?;
    trace!("block_type: {:02b}", block_type);
    debug_assert!(block_type < 0b100);

    if block_type == 0b00 {
      trace!("uncompressed block");
      todo!("handle no-compression")
    } else {
      let (lit_len_alphabet, dist_alphabet) = if block_type == 0b11 {
        trace!("reading dynamic tree data");
        todo!("read dynamic tree")
      } else {
        trace!("using fixed tree data");
        let mut dist_alphabet = DistAlphabet::default();
        dist_alphabet.tree.iter_mut().for_each(|m| m.bit_count = 5);
        dist_alphabet.refresh();
        (FIXED_HUFFMAN_TREE, dist_alphabet)
      };
      trace!("{:?}", bit_src);
      'per_symbol: loop {
        trace!("pre-lit-len: {:?}", bit_src);
        let lit_len = lit_len_alphabet.pull_and_match(bit_src)?;
        match lit_len {
          lit @ 0..=255 => {
            if out_position < out.len() {
              trace!("pushing literal '{}'", lit);
              out[out_position] = lit as u8;
              out_position += 1;
            } else {
              return Err(PngError::OutputOverflow);
            }
          }
          256 => break 'per_symbol,
          len_symbol => {
            debug_assert!(lit_len <= 285);
            let len = if len_symbol <= 264 {
              len_symbol - 254
            } else if len_symbol <= 284 {
              let num_extra_bits = (len_symbol - 261) / 4;
              ((len_symbol - 265) % 4 + 4)
                << num_extra_bits + 3 + bit_src.next_bits_lsb(num_extra_bits as u32)?
            } else {
              258
            };
            trace!("back ref len: {}", len);
            let dist = {
              let dist_sym = dist_alphabet.pull_and_match(bit_src)?;
              debug_assert!(dist_sym <= 29);
              if dist_sym <= 3 {
                dist_sym + 1
              } else if dist_sym <= 29 {
                let num_extra_bits = dist_sym / 2 - 1;
                ((dist_sym % 2 + 2) << num_extra_bits)
                  + 1
                  + bit_src.next_bits_lsb(num_extra_bits as u32)?
              } else {
                panic!("illegal dist sym from the dist_alphabet")
              }
            };
            trace!("back ref dist: {}", dist);
            trace!("pushing back ref: <{}, {}>", len, dist);
            let start_of_back_dist = out_position
              .checked_sub(dist)
              .ok_or(PngError::BackRefToBeforeOutputStart)?;
            trace!("start position of back ref: {}", start_of_back_dist);
            if out_position + len <= out.len() {
              let mut d = start_of_back_dist;
              (0..len).for_each(|_| {
                out[out_position] = out[d];
                out_position += 1;
                d += 1;
              });
            } else {
              return Err(PngError::OutputOverflow);
            }
          }
        }
      }
    }

    if is_final_block {
      return Ok(());
    }
  }
}
