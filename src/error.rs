use core::{
  num::{ParseIntError, TryFromIntError},
  str::Utf8Error,
};

/// An error from the `imagine` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagineError {
  /// Failed to parse the data given.
  Parse,

  /// The allocator couldn't give us enough space.
  #[cfg(feature = "alloc")]
  Alloc,

  /// The image is too large.
  ///
  /// The automatic decoder limits the width and height of images it processes
  /// to be 17,000 or less to prevent accidental out-of-memory problems.
  DimensionsTooLarge,

  /// The declared width and/or height of this image is 0.
  WidthOrHeightZero,

  /// A checked math operation failed.
  CheckedMath,
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
    Self::Parse
  }
}
