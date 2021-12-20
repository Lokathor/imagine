#![forbid(unsafe_code)]

//! This module gives support for the various
//! [Netpbm](https://en.wikipedia.org/wiki/Netpbm) formats.
//!
//! Several file extensions are used by this format family: `.pbm`, `.pgm`,
//! `.ppm`, `.pnm`, and `.pam`. They're all extremely simple formats with
//! absolutely no compression.
//!
//! First use [`netpbm_parse_header`] to get the header information and the
//! pixel data bytes. This gives you the size of the image, and also tells you
//! how to interpret the pixel bytes. Then you can use the appropriate iterator
//! on the pixel bytes to decode all the pixel values.
//!
//! Important: The colorspace of a Netpbm file is never given in the header.
//! Instead, you have to guess at what color space the data is intended for.
//! * Color images will *often* use [CIE Rec. 709](https://en.wikipedia.org/wiki/Rec._709),
//!   but might be using sRGB, or they might even be linear. The "CIE Rec. 709"
//!   colorspace is *similar* to sRGB with a slightly different gamma curve, so
//!   mostly you can assume sRGB and it'll work often enough.
//! * Monochrome images are *often* in linear space, but might use sRGB.
//! * There are also 1-bit-per-pixel images, but since they are always either
//!   the minimum value they're effectively color space independent.

mod iter_ascii_1bpp;
pub use iter_ascii_1bpp::*;

mod iter_ascii_u8;
pub use iter_ascii_u8::*;

mod iter_ascii_u16;
pub use iter_ascii_u16::*;

mod automatic;
pub use automatic::*;

/// Parses for a netpbm header, along with the pixel data.
///
/// * **Success:** Returns the header and pixel data slice. The header contains
///   the dimensions of the image and a `NetpbmDataFormat` value that describes
///   how you interpret the rest of the pixel data.
/// * **Failure:** Returns the parsing error.
pub fn netpbm_parse_header(netpbm: &[u8]) -> Result<(NetpbmHeader, &[u8]), NetpbmError> {
  const U8_MAX_AS_U32: u32 = u8::MAX as u32;
  const U16_MAX_AS_U32: u32 = u16::MAX as u32;
  //
  let (tag, rest) = netpbm_break_tag(netpbm)?;
  match tag {
    NetpbmTag::P7 => {
      let (width, rest) = match netpbm_trim_comments_and_whitespace(rest) {
        [b'W', b'I', b'D', b'T', b'H', b' ', rest @ ..] => netpbm_read_ascii_unsigned(rest)?,
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      let (height, rest) = match netpbm_trim_comments_and_whitespace(rest) {
        [b'H', b'E', b'I', b'G', b'H', b'T', b' ', rest @ ..] => netpbm_read_ascii_unsigned(rest)?,
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      let (depth, rest) = match netpbm_trim_comments_and_whitespace(rest) {
        [b'D', b'E', b'P', b'T', b'H', b' ', rest @ ..] => netpbm_read_ascii_unsigned(rest)?,
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      let (max, rest) = match netpbm_trim_comments_and_whitespace(rest) {
        [b'M', b'A', b'X', b'V', b'A', b'L', b' ', rest @ ..] => netpbm_read_ascii_unsigned(rest)?,
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      if max > U16_MAX_AS_U32 {
        return Err(NetpbmError::MaxValueExceedsU16);
      }
      let (data_format, rest) = match netpbm_trim_comments_and_whitespace(rest) {
        [b'T', b'U', b'P', b'L', b'T', b'Y', b'P', b'E', b' ', rest @ ..] => {
          let mut splitn_it = rest.splitn(2, |u| u.is_ascii_whitespace());
          let channel_layout =
            match (splitn_it.next().ok_or(NetpbmError::HeaderIllegalFormat)?, depth, max) {
              (b"BLACKANDWHITE", 1, 1) => NetpbmDataFormat::Binary_Y_U8 { max: 1 },
              (b"GRAYSCALE", 1, _) => {
                if max <= U8_MAX_AS_U32 {
                  NetpbmDataFormat::Binary_Y_U8 { max: max as u8 }
                } else {
                  NetpbmDataFormat::Binary_Y_U16BE { max: max as u16 }
                }
              }
              (b"RGB", 3, _) => {
                if max <= U8_MAX_AS_U32 {
                  NetpbmDataFormat::Binary_RGB_U8 { max: max as u8 }
                } else {
                  NetpbmDataFormat::Binary_RGB_U16BE { max: max as u16 }
                }
              }
              (b"BLACKANDWHITE_ALPHA", 2, 1) => NetpbmDataFormat::Binary_Y_U8 { max: 1 },
              (b"GRAYSCALE_ALPHA", 2, _) => {
                if max <= U8_MAX_AS_U32 {
                  NetpbmDataFormat::Binary_YA_U8 { max: max as u8 }
                } else {
                  NetpbmDataFormat::Binary_YA_U16BE { max: max as u16 }
                }
              }
              (b"RGB_ALPHA", 4, _) => {
                if max <= U8_MAX_AS_U32 {
                  NetpbmDataFormat::Binary_RGBA_U8 { max: max as u8 }
                } else {
                  NetpbmDataFormat::Binary_RGBA_U16BE { max: max as u16 }
                }
              }
              _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
            };
          let rest = splitn_it.next().ok_or(NetpbmError::HeaderIllegalFormat)?;
          (channel_layout, rest)
        }
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      let pixel_data = match netpbm_trim_comments_and_whitespace(rest) {
        [b'E', b'N', b'D', b'H', b'D', b'R', b'\n', rest @ ..] => rest,
        [b'E', b'N', b'D', b'H', b'D', b'R', b'\r', b'\n', rest @ ..] => rest,
        _otherwise => return Err(NetpbmError::HeaderIllegalFormat),
      };
      //
      Ok((NetpbmHeader { width, height, data_format }, pixel_data))
    }
    _ => {
      let (width, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
      let (height, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
      match tag {
        NetpbmTag::P1 => {
          let pixel_data = netpbm_trim_comments_and_whitespace(rest);
          //
          Ok((
            NetpbmHeader { width, height, data_format: NetpbmDataFormat::Ascii_Y_1bpp },
            pixel_data,
          ))
        }
        NetpbmTag::P2 => {
          let (max, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = netpbm_trim_comments_and_whitespace(rest);
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max <= U8_MAX_AS_U32 {
                NetpbmDataFormat::Ascii_Y_U8 { max: max as u8 }
              } else {
                NetpbmDataFormat::Ascii_Y_U16 { max: max as u16 }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::P3 => {
          let (max, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = netpbm_trim_comments_and_whitespace(rest);
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max <= U8_MAX_AS_U32 {
                NetpbmDataFormat::Ascii_RGB_U8 { max: max as u8 }
              } else {
                NetpbmDataFormat::Ascii_RGB_U16 { max: max as u16 }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::P4 => {
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader { width, height, data_format: NetpbmDataFormat::Binary_Y_1bpp },
            pixel_data,
          ))
        }
        NetpbmTag::P5 => {
          let (max, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max <= U8_MAX_AS_U32 {
                NetpbmDataFormat::Binary_Y_U8 { max: max as u8 }
              } else {
                NetpbmDataFormat::Binary_Y_U16BE { max: max as u16 }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::P6 => {
          let (max, rest) = netpbm_read_ascii_unsigned(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max <= U8_MAX_AS_U32 {
                NetpbmDataFormat::Binary_RGB_U8 { max: max as u8 }
              } else {
                NetpbmDataFormat::Binary_RGB_U16BE { max: max as u16 }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::Pf => {
          let (max, rest) = netpbm_read_ascii_float(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max < 0.0 {
                NetpbmDataFormat::Binary_Y_F32LE { max: -max }
              } else {
                NetpbmDataFormat::Binary_Y_F32BE { max }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::PF => {
          let (max, rest) = netpbm_read_ascii_float(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max < 0.0 {
                NetpbmDataFormat::Binary_RGB_F32LE { max: -max }
              } else {
                NetpbmDataFormat::Binary_RGB_F32BE { max }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::PF4 => {
          let (max, rest) = netpbm_read_ascii_float(netpbm_trim_comments_and_whitespace(rest))?;
          let pixel_data = &rest[1..];
          //
          Ok((
            NetpbmHeader {
              width,
              height,
              data_format: if max < 0.0 {
                NetpbmDataFormat::Binary_RGBA_F32LE { max: -max }
              } else {
                NetpbmDataFormat::Binary_RGBA_F32BE { max }
              },
            },
            pixel_data,
          ))
        }
        NetpbmTag::P7 => unreachable!("the outer match covers this case"),
      }
    }
  }
}

/// Things that might go wrong when parsing a Netpbm file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum NetpbmError {
  /// There's no Netpbm tag, so probably this isn't even a Netpbm file.
  NoTagPresent,
  /// Not enough bytes are in the file.
  InsufficientBytes,
  /// An ascii string couldn't be parsed as a decimal unsigned value.
  CouldNotParseUnsigned,
  /// An ascii string couldn't be parsed as an `f32` value.
  CouldNotParseFloat,
  /// An allocation failed.
  CouldNotAlloc,
  /// The file's header declared a maximum value exceeds `u16::MAX`, which this
  /// library doesn't support.
  MaxValueExceedsU16,
  /// While parsing a file, a particular integer entry exceeded the maximum
  /// value declared in the file's header.
  IntegerExceedsMaxValue,
  /// The header format didn't confrom to expectations in some way.
  HeaderIllegalFormat,
}
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for NetpbmError {
  fn from(_: alloc::collections::TryReserveError) -> Self {
    NetpbmError::CouldNotAlloc
  }
}

/// Header for a Netpbm file where channel values are stored as integers.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NetpbmHeader {
  /// Image width in pixels.
  pub width: u32,
  /// Image height in pixels.
  pub height: u32,
  /// Format of the pixel data.
  pub data_format: NetpbmDataFormat,
}

/// The data format of the pixels in a netpbm file.
///
/// There's unfortunately quite a few possibilities.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[allow(non_camel_case_types)]
#[rustfmt::skip]
pub enum NetpbmDataFormat {
  /// Ascii, 1-bit-per-pixel.
  ///
  /// In this format, all pixels are 0 ("pen up") or 1 ("pen down"). It's
  /// somewhat implied that a black pen is drawing on a white surface, but
  /// that's up to you I guess.
  Ascii_Y_1bpp,
  Ascii_Y_U8 { max: u8 },
  Ascii_Y_U16 { max: u16 },
  Ascii_RGB_U8 { max: u8 },
  Ascii_RGB_U16 { max: u16 },
  /// Binary, 1-bit-per-pixel (bit packed, high to low).
  ///
  /// In this format, all pixels are 0 ("pen up") or 1 ("pen down"). It's
  /// somewhat implied that a black pen is drawing on a white surface, but
  /// that's up to you I guess.
  Binary_Y_1bpp,
  Binary_Y_U8 { max: u8 },
  Binary_Y_U16BE { max: u16 },
  Binary_Y_F32BE { max: f32 },
  Binary_Y_F32LE { max: f32 },
  Binary_YA_U8 { max: u8 },
  Binary_YA_U16BE { max: u16 },
  Binary_RGB_U8 { max: u8 },
  Binary_RGB_U16BE { max: u16 },
  Binary_RGB_F32BE { max: f32 },
  Binary_RGB_F32LE { max: f32 },
  Binary_RGBA_U8 { max: u8 },
  Binary_RGBA_U16BE { max: u16 },
  Binary_RGBA_F32BE { max: f32 },
  Binary_RGBA_F32LE { max: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum NetpbmTag {
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
fn netpbm_trim_comments_and_whitespace(mut bytes: &[u8]) -> &[u8] {
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
fn netpbm_break_tag(netpbm: &[u8]) -> Result<(NetpbmTag, &[u8]), NetpbmError> {
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

fn netpbm_read_ascii_unsigned(bytes: &[u8]) -> Result<(u32, &[u8]), NetpbmError> {
  let (digits, rest) =
    bytes.split_at(bytes.iter().position(|u| u.is_ascii_whitespace()).unwrap_or(bytes.len()));
  let u: u32 = core::str::from_utf8(digits)
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?
    .parse()
    .map_err(|_| NetpbmError::CouldNotParseUnsigned)?;
  Ok((u, rest))
}

fn netpbm_read_ascii_float(bytes: &[u8]) -> Result<(f32, &[u8]), NetpbmError> {
  let (digits, rest) =
    bytes.split_at(bytes.iter().position(|u| u.is_ascii_whitespace()).unwrap_or(bytes.len()));
  let f: f32 = core::str::from_utf8(digits)
    .map_err(|_| NetpbmError::CouldNotParseFloat)?
    .parse()
    .map_err(|_| NetpbmError::CouldNotParseFloat)?;
  Ok((f, rest))
}
