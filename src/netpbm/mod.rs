#![forbid(unsafe_code)]

//! This module gives support for the various
//! [Netpbm](https://en.wikipedia.org/wiki/Netpbm) formats.
//!
//! Several file extensions are used by this format family: `.pbm`, `.pgm`,
//! `.ppm`, `.pnm`, and `.pam`. They're all extremely formats with absolutely no
//! compression.
//!
//! These formats always start as ascii data, but sometimes transition to binary
//! data. As long as you're in an ascii portion of the file, a `#` can begin a
//! comment until the end of the line. Ascii data considers any amount of
//! whitespace between elements, including line breaks, to be equivalent.
//!
//! TODO: basics?
//!
//! Important: The colorspace of a Netpbm file is never given in the header.
//! * Color images are *often* in [CIE Rec. 709](https://en.wikipedia.org/wiki/Rec._709),
//!   but might be sRGB, or might be linear. The "CIE Rec. 709" colorspace is
//!   *similar* to sRGB with a slightly different gamma curve.
//! * Monochrome images are *often* in linear color, but might use sRGB.
//! * There are also 1-bit-per-pixel images, but they obviously don't use a
//!   color space.
//!
//! ## `P1` to `P6`
//!
//! These formats use ascii (1-3) or big-endian binary integer (4-6) data.
//! * P1 and P4 are 1-bit-per-pixel.
//! * P2 and P5 are monochrome.
//! * P3 and P6 are is RGB.
//!
//! The format is quite simple:
//! * Tag (ascii)
//! * Width (ascii)
//! * Height (ascii)
//! * Unless it's P1 or P4: Maximum Value (ascii) `1..=u16::MAX`
//! * Pixel data (ascii or binary, according to tag)
//!
//! Note that P1 and P4 are 1-bit-per-pixel formats, so they don't state their
//! maximum value within the header.
//!
//! * Ascii data is parsed as unsigned integers, consuming however many
//!   characters each.
//! * Binary monochrome and RGB data is 1 byte per channel value if the maximum
//!   is 255 or less, and 2 byte per channel (big-endian) if the maximum is 256
//!   or more.
//! * P1 and P4 are slightly special.
//!   * P1 uses one character per pixel and does *not* require whitespace
//!     between characters. Eg: `0 0` or `00` are both two 0 pixels.
//!   * P4 makes the pixels be bit-packed into the bytes, high to low.
//!
//! ## `Pf`, `PF`, and `PF4`
//!
//! These formats always use binary `f32` data to specify the pixel channel
//! values.
//! * Pf is monochrome
//! * PF is RGB
//! * PF4 is RGBA
//!
//! The header is basically as with the P1 through P6 forms: Tag, Width, Height,
//! Maximum, then pixel data. The important difference is that the "maximum"
//! value *can* be negative rathe than positive.
//! * In terms of the range for channel values, only the absolute value is
//!   significant.
//! * However, when the maximum is negative then the `f32` byte order switches
//!   to little-endian.
//!
//! ## `P7`
//!
//! This format is also binary-only, but unlike with the previous forms the
//! channels aren't fixed.
//!
//! The header is slightly more complicated and regimented:
//! * "P7"
//! * "WIDTH"
//! * image width (ascii)
//! * "DEPTH"
//! * image channel count (ascii), must match what `TUPLTYPE` specifies.
//! * "MAXVAL"
//! * image maximum value (ascii)
//! * "TUPLTYPE"
//! * Image channel layout, one of the following constant names:
//!   * `BLACKANDWHITE` (monochrome, but the `maxval` is supposed to be 1)
//!   * `GRAYSCALE`
//!   * `RGB`
//!   * `BLACKANDWHITE_ALPHA` (as above, plus alpha)
//!   * `GRAYSCALE_ALPHA` (as above, plus alpha)
//!   * `RGB_ALPHA` (as above, plus alpha)
//! * "ENDHDR"
//! * Pixel data
//!
//! Each label and value is supposed to appear on its own line. The P7 format
//! does not support comments.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum NetpbmError {
  NoTagPresent,
  InsufficientBytes,
  CouldNotParseUnsigned,
  CouldNotParseFloat,
  CouldNotAlloc,
  MaxValueExceedsU16,
  IntegerExceedsMaxValue,
  HeaderIllegalFormat,
}

/// Describes what channels are in the Netpbm image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetpbmChannels {
  Y,
  YA,
  RGB,
  RGBA,
}

/// Describes how the data for each channel is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetpbmDataFormat {
  /// Data is ascii strings that have to be parsed.
  ///
  /// Values are normally whitespace separated. If the image is gray scale with
  /// a max value of 1 (aka the P1 tag) then whitespace is not required.
  Ascii,
  /// Data is binary bytes.
  ///
  /// Depending on the maximum value per channel there might be more than one
  /// byte per channel value. Bytes should be read as big-endian by default.
  Binary,
  /// Pixel values are 1 bit per pixel and packed into bytes, high bit to low
  /// bit.
  Bitpacked,
}

/// Header for a Netpbm file where channel values are stored as integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetpbmIntHeader {
  /// Image width in pixels.
  pub width: u32,
  /// Image height in pixels.
  pub height: u32,
  /// The maximum value in a single channel.
  pub max_value: u32,
  /// The channel layout of the image.
  pub channels: NetpbmChannels,
  /// The layout of the channel data within the file.
  pub data_format: NetpbmDataFormat,
}

/// Header for a Netpbm file where channel values are stored as floats.
///
/// For this header, pixel data is always binary, but depending on the
/// `max_value` that the header declares the floats can be either big-endian or
/// little-endian.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NetpbmFloatHeader {
  /// Image width in pixels.
  pub width: u32,
  /// Image height in pixels.
  pub height: u32,
  /// The absolute value indicates the range, while sign indicates data
  /// endian-ness.
  ///
  /// * Positive: big-endian.
  /// * Negative: little-endian.
  pub max_value: f32,
  /// The channel layout of the image.
  pub channels: NetpbmChannels,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum NetpbmHeader {
  Int(NetpbmIntHeader),
  Float(NetpbmFloatHeader),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetpbmTag {
  P1,
  P2,
  P3,
  P4,
  P5,
  P6,
  P7,
  Pf,
  PF,
  PF4,
}

/// Trims off any comments and whitespace at the start of the bytes.
///
/// This makes the bytes ready to read in the next token.
pub fn netpbm_trim_comments_and_whitespace(mut bytes: &[u8]) -> &[u8] {
  loop {
    match bytes.get(0) {
      // Note(Lokathor): A '#' starts a comment to the end of the "line". By
      // looking for just '\n' will cover both unix ("\n") and windows ("\r\n")
      // style line endings. If someone wants to improve the line ending logic
      // they can PR that.
      Some(b'#') => match bytes.iter().position(|&u| u == b'\n') {
        Some(i) => bytes = &bytes[i..],
        None => bytes = &[],
      },
      // Note(Lokathor): According to the docs on this method not everyone
      // agrees what exactly "ascii whitespace" is, but the Netpbm file format
      // family isn't super hard specified so people shouldn't be getting cute
      // with it anyway.
      Some(u) if u.is_ascii_whitespace() => bytes = &bytes[1..],
      _ => break,
    }
  }
  bytes
}

/// Breaks the tag off the front of the bytes.
///
/// Also trims off any comments and whitespace so that you're immediately ready
/// to read the next token.
pub fn netpbm_break_tag(netpbm: &[u8]) -> Result<(NetpbmTag, &[u8]), NetpbmError> {
  Ok(match netpbm {
    [b'P', b'1', rest @ ..] => (NetpbmTag::P1, rest),
    [b'P', b'2', rest @ ..] => (NetpbmTag::P2, rest),
    [b'P', b'3', rest @ ..] => (NetpbmTag::P3, rest),
    [b'P', b'4', rest @ ..] => (NetpbmTag::P4, rest),
    [b'P', b'5', rest @ ..] => (NetpbmTag::P5, rest),
    [b'P', b'6', rest @ ..] => (NetpbmTag::P6, rest),
    [b'P', b'7', rest @ ..] => (NetpbmTag::P7, rest),
    [b'P', b'f', rest @ ..] => (NetpbmTag::Pf, rest),
    [b'P', b'F', b'4', rest @ ..] => (NetpbmTag::PF4, rest),
    [b'P', b'F', rest @ ..] => (NetpbmTag::PF, rest),
    _ => return Err(NetpbmError::NoTagPresent),
  })
}

pub fn netpbm_read_ascii_unsigned(bytes: &[u8]) -> Result<(u32, &[u8]), NetpbmError> {
  let (digits, rest) =
    bytes.split_at(bytes.iter().position(|u| u.is_ascii_whitespace()).unwrap_or(bytes.len()));
  let u: u32 = core::str::from_utf8(digits)
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?
    .parse()
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?;
  Ok((u, rest))
}

pub fn netpbm_read_ascii_float(bytes: &[u8]) -> Result<(f32, &[u8]), NetpbmError> {
  let (digits, rest) =
    bytes.split_at(bytes.iter().position(|u| u.is_ascii_whitespace()).unwrap_or(bytes.len()));
  let f: f32 = core::str::from_utf8(digits)
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?
    .parse()
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?;
  Ok((f, rest))
}
