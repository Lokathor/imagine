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
    if cfg!(feature = "trace") {
      print!("{file}:{line}> ", file = file!(), line = line!());
      println!($($arg)*);
    }
  }
}

mod chunk;
pub use chunk::*;

mod chunk_iter;
pub use chunk_iter::*;

mod png_header;
pub use png_header::*;

mod decompress;
pub use decompress::decompress_idat_to;

mod filtering;
pub use filtering::reconstruct_in_place;

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
  LenAndNLenDidNotMatch,
  BadDynamicHuffmanTreeData,
  InterlaceNotSupported,
  IllegalColorTypeBitDepthCombination,
  TempMemoryWrongSizeForHeader,
  NotAnIhdrChunk,
  IllegalWidthZero,
  IllegalHeightZero,
  IllegalFilterMethod,
  IllegalAdaptiveFilterType,
}
