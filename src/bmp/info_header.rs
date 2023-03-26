use super::*;
use crate::util::*;

/// An enum over the various BMP info header versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum BmpInfoHeader {
  Core(BmpInfoHeaderCore),
  Os22x(BmpInfoHeaderOs22x),
  V1(BmpInfoHeaderV1),
  V2(BmpInfoHeaderV2),
  V3(BmpInfoHeaderV3),
  V4(BmpInfoHeaderV4),
  V5(BmpInfoHeaderV5),
}
impl BmpInfoHeader {
  /// Tries to get the info header and remaining bytes.
  #[inline]
  pub fn try_from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), BmpError> {
    if bytes.len() < 4 {
      return Err(BmpError::InsufficientBytes);
    }
    Ok(match u32_le(&bytes[0..4]) {
      12 => {
        let (a, rest) = try_pull_byte_array::<12>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::Core(BmpInfoHeaderCore::try_from(a)?), rest)
      }
      16 => {
        let (a, rest) = try_pull_byte_array::<16>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::Os22x(BmpInfoHeaderOs22x::try_from(a)?), rest)
      }
      64 => {
        let (a, rest) = try_pull_byte_array::<64>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::Os22x(BmpInfoHeaderOs22x::try_from(a)?), rest)
      }
      40 => {
        let (a, rest) = try_pull_byte_array::<40>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::V1(BmpInfoHeaderV1::try_from(a)?), rest)
      }
      52 => {
        let (a, rest) = try_pull_byte_array::<52>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::V2(BmpInfoHeaderV2::try_from(a)?), rest)
      }
      56 => {
        let (a, rest) = try_pull_byte_array::<56>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::V3(BmpInfoHeaderV3::try_from(a)?), rest)
      }
      108 => {
        let (a, rest) =
          try_pull_byte_array::<108>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::V4(BmpInfoHeaderV4::try_from(a)?), rest)
      }
      124 => {
        let (a, rest) =
          try_pull_byte_array::<124>(bytes).ok().ok_or(BmpError::InsufficientBytes)?;
        (Self::V5(BmpInfoHeaderV5::try_from(a)?), rest)
      }
      _ => return Err(BmpError::UnknownHeaderLength),
    })
  }

  /// Image pixel width.
  #[inline]
  #[must_use]
  pub const fn width(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { width, .. }) => width as i32,
      Self::Os22x(BmpInfoHeaderOs22x { width, .. })
      | Self::V1(BmpInfoHeaderV1 { width, .. })
      | Self::V2(BmpInfoHeaderV2 { width, .. })
      | Self::V3(BmpInfoHeaderV3 { width, .. })
      | Self::V4(BmpInfoHeaderV4 { width, .. })
      | Self::V5(BmpInfoHeaderV5 { width, .. }) => width,
    }
  }

  /// Image pixel height.
  ///
  /// * A positive height indicates that the origin is the **bottom** left.
  /// * A negative height indicates that the image origin is the **top** left.
  #[inline]
  #[must_use]
  pub const fn height(self) -> i32 {
    match self {
      Self::Core(BmpInfoHeaderCore { height, .. }) => height as i32,
      Self::Os22x(BmpInfoHeaderOs22x { height, .. })
      | Self::V1(BmpInfoHeaderV1 { height, .. })
      | Self::V2(BmpInfoHeaderV2 { height, .. })
      | Self::V3(BmpInfoHeaderV3 { height, .. })
      | Self::V4(BmpInfoHeaderV4 { height, .. })
      | Self::V5(BmpInfoHeaderV5 { height, .. }) => height,
    }
  }

  /// Bits per pixel, should be in the 1 to 32 range.
  #[inline]
  #[must_use]
  pub const fn bits_per_pixel(self) -> u16 {
    match self {
      Self::Core(BmpInfoHeaderCore { bits_per_pixel, .. })
      | Self::Os22x(BmpInfoHeaderOs22x { bits_per_pixel, .. })
      | Self::V1(BmpInfoHeaderV1 { bits_per_pixel, .. })
      | Self::V2(BmpInfoHeaderV2 { bits_per_pixel, .. })
      | Self::V3(BmpInfoHeaderV3 { bits_per_pixel, .. })
      | Self::V4(BmpInfoHeaderV4 { bits_per_pixel, .. })
      | Self::V5(BmpInfoHeaderV5 { bits_per_pixel, .. }) => bits_per_pixel,
    }
  }

  /// Compression method.
  #[inline]
  #[must_use]
  pub const fn compression(self) -> BmpCompression {
    match self {
      Self::Core(BmpInfoHeaderCore { .. }) => BmpCompression::RgbNoCompression,
      Self::Os22x(BmpInfoHeaderOs22x { compression, .. })
      | Self::V1(BmpInfoHeaderV1 { compression, .. })
      | Self::V2(BmpInfoHeaderV2 { compression, .. })
      | Self::V3(BmpInfoHeaderV3 { compression, .. })
      | Self::V4(BmpInfoHeaderV4 { compression, .. })
      | Self::V5(BmpInfoHeaderV5 { compression, .. }) => compression,
    }
  }

  /// Gets the number of palette entries.
  ///
  /// The meaning of a `None` value for the `palette_len` field on the wrapped
  /// structures changes depending on the bit depth of the image, so this method
  /// handles that difference for you and just gives you a single value that's
  /// the *actual* number of entries on the palette.
  #[inline]
  #[must_use]
  pub const fn palette_len(self) -> usize {
    match self {
      Self::Core(BmpInfoHeaderCore { bits_per_pixel, .. }) => 1 << bits_per_pixel,
      Self::Os22x(x) => x.palette_len(),
      Self::V1(x) => x.palette_len(),
      Self::V2(x) => x.palette_len(),
      Self::V3(x) => x.palette_len(),
      Self::V4(x) => x.palette_len(),
      Self::V5(x) => x.palette_len(),
    }
  }

  /// Gets the number of bytes in the pixel data region of the file.
  #[inline]
  #[must_use]
  pub const fn pixel_data_len(self) -> usize {
    match self {
      Self::Core(BmpInfoHeaderCore { .. }) => {
        self.width().unsigned_abs().saturating_mul(self.height().unsigned_abs()) as usize
      }
      Self::Os22x(BmpInfoHeaderOs22x { image_byte_size, .. })
      | Self::V1(BmpInfoHeaderV1 { image_byte_size, .. })
      | Self::V2(BmpInfoHeaderV2 { image_byte_size, .. })
      | Self::V3(BmpInfoHeaderV3 { image_byte_size, .. })
      | Self::V4(BmpInfoHeaderV4 { image_byte_size, .. })
      | Self::V5(BmpInfoHeaderV5 { image_byte_size, .. }) => match image_byte_size {
        Some(x) => x.get() as usize,
        None => {
          let width_u = self.width().unsigned_abs() as usize;
          let height_u = self.height().unsigned_abs() as usize;
          let bits_per_line = width_u.saturating_mul(self.bits_per_pixel() as usize);
          let bytes_per_line_no_padding =
            (bits_per_line / 8) + (((bits_per_line % 8) != 0) as usize);
          let bytes_per_line_padded = ((bytes_per_line_no_padding / 4)
            + (((bytes_per_line_no_padding % 4) != 0) as usize))
            .saturating_mul(4);
          height_u.saturating_mul(bytes_per_line_padded)
        }
      },
    }
  }

  /// If the image is supposed to be sRGB colors or not.
  #[inline]
  pub const fn is_srgb(self) -> bool {
    match self {
      BmpInfoHeader::Core(_) => false,
      BmpInfoHeader::Os22x(_) => false,
      BmpInfoHeader::V1(_) => false,
      BmpInfoHeader::V2(_) => false,
      BmpInfoHeader::V3(_) => false,
      BmpInfoHeader::V4(BmpInfoHeaderV4 { colorspace, .. }) => {
        matches!(colorspace, BmpColorspace::Srgb | BmpColorspace::WindowsDefault)
      }
      BmpInfoHeader::V5(BmpInfoHeaderV5 { srgb_intent, colorspace, .. }) => {
        srgb_intent.is_some()
          || matches!(colorspace, BmpColorspace::Srgb | BmpColorspace::WindowsDefault)
      }
    }
  }
}
