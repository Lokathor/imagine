use core::{
  num::{ParseIntError, TryFromIntError},
  str::Utf8Error,
};

/// An error from the `imagine` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagineError {
  /// The allocator couldn't give us enough space.
  #[cfg(feature = "alloc")]
  Alloc,

  /// The image is too large.
  ///
  /// The automatic decoder limits the width and height of images it processes
  /// to be 17,000 or less to prevent accidental out-of-memory problems.
  DimensionsTooLarge,

  /// Failed to parse the data given.
  Parse,

  /// The parsing completed properly, but one or more values had an illegal
  /// combination.
  ///
  /// For example, the width or height might be 0, or the bit depth and
  /// compression format might be an illegal combination.
  Value,

  /// There's (probably) nothing wrong with your file, but the library can't
  /// handle it because some part of the decoder is incomplete.
  ///
  /// As an example: the netpbm parser can't handle the P7 format.
  IncompleteLibrary,
}
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for ImagineError {
  #[inline]
  fn from(_: alloc::collections::TryReserveError) -> Self {
    Self::Alloc
  }
}
impl From<Utf8Error> for ImagineError {
  #[inline]
  fn from(_: Utf8Error) -> Self {
    Self::Parse
  }
}
impl From<ParseIntError> for ImagineError {
  #[inline]
  fn from(_: ParseIntError) -> Self {
    Self::Parse
  }
}
impl From<TryFromIntError> for ImagineError {
  #[inline]
  fn from(_: TryFromIntError) -> Self {
    Self::Value
  }
}
