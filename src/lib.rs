#![cfg_attr(not(feature = "trace"), no_std)]
#![forbid(unsafe_code)]
//#![warn(missing_docs)]

use core::{convert::TryInto, iter::Iterator};

#[cfg(feature = "trace")]
extern crate std;

#[macro_export]
macro_rules! trace {
  ($($arg:tt)*) => {
    #[cfg(feature = "trace")] {
      ::std::print!("{file}:{line}> ", file = file!(), line = line!());
      ::std::println!($($arg)*);
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
#[allow(missing_docs)]
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
