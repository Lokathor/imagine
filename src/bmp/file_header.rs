use super::*;
use crate::{ascii_array::*, util::*};

/// Two-letter file tags commonly found at the start of a BMP file.
pub const COMMON_BMP_TAGS: &[AsciiArray<2>] = &[
  AsciiArray(*b"BM"),
  AsciiArray(*b"BA"),
  AsciiArray(*b"CI"),
  AsciiArray(*b"CP"),
  AsciiArray(*b"IC"),
  AsciiArray(*b"PT"),
];

/// The header at the start of all BMP files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BmpFileHeader {
  /// This is expected to be one of the following
  ///
  /// * BM: win3.1 or later
  /// * BA: OS/2 bitmap array
  /// * CI: OS/2 color icon
  /// * CP: OS/2 color pointer
  /// * IC: OS/2 icon
  /// * PT: OS/2 pointer
  pub tag: AsciiArray<2>,

  /// The total size of the file.
  ///
  /// If this doesn't match the actual size of the file, there might be some
  /// sort of data loss or corruption.
  pub total_file_size: u32,

  /// The byte index within the file where the bitmap data starts.
  pub pixel_data_offset: u32,
}
impl From<[u8; 14]> for BmpFileHeader {
  #[inline]
  #[must_use]
  fn from(value: [u8; 14]) -> Self {
    Self {
      tag: AsciiArray(value[0..2].try_into().unwrap()),
      total_file_size: u32_le(&value[2..6]),
      // 4 bytes skipped
      pixel_data_offset: u32_le(&value[10..14]),
    }
  }
}
impl From<BmpFileHeader> for [u8; 14] {
  #[inline]
  fn from(h: BmpFileHeader) -> Self {
    let mut a = [0; 14];
    a[0..2].copy_from_slice(h.tag.0.as_slice());
    a[2..6].copy_from_slice(h.total_file_size.to_le_bytes().as_slice());
    // 4 bytes are left blank
    a[10..14].copy_from_slice(h.pixel_data_offset.to_le_bytes().as_slice());
    a
  }
}
impl BmpFileHeader {
  /// Tries to get the file header and remaining bytes from the bytes of a BMP
  /// file.
  ///
  /// The bytes from here should be used to get the [BmpInfoHeader].
  #[inline]
  pub fn try_from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), BmpError> {
    let (a, rest) = try_pull_byte_array::<14>(bytes).ok_or(BmpError::InsufficientBytes)?;
    Ok((Self::from(a), rest))
  }
}
