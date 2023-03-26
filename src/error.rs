use core::{num::ParseIntError, str::Utf8Error};

/// An error from the `imagine` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagineError {
  /// The allocator couldn't give us enough space.
  #[cfg(feature = "alloc")]
  AllocError,
  /// Failed to parse the data given.
  ///
  /// For any particular file format, all sorts of things could go wrong.
  ParseError,
}
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for ImagineError {
  #[inline]
  fn from(_: alloc::collections::TryReserveError) -> Self {
    Self::AllocError
  }
}
impl From<Utf8Error> for ImagineError {
  #[inline]
  fn from(_: Utf8Error) -> Self {
    Self::ParseError
  }
}
impl From<ParseIntError> for ImagineError {
  #[inline]
  fn from(_: ParseIntError) -> Self {
    Self::ParseError
  }
}
