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

use pixel_formats::r8g8b8a8_Srgb;

use crate::image::Bitmap;

/// Parses for a netpbm header, along with the pixel data.
///
/// * **Success:** Returns the header and pixel data slice. The header contains
///   the dimensions of the image and a `NetpbmDataFormat` value that describes
///   how you interpret the rest of the pixel data.
/// * **Failure:** Returns the parsing error.
#[allow(clippy::missing_inline_in_public_items)]
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
  /// The header format didn't conform to expectations in some way.
  HeaderIllegalFormat,
}
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for NetpbmError {
  #[inline]
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

/// Parses 1bpp ascii entries.
///
/// For the purposes of this parse whitespace and comments are skipped over as
/// usual, and in addition whitespace between entries is not required.
/// * Each `b'0'` outputs as `Ok(false)`
/// * Each `b'1'` outputs as `Ok(true)`
/// * Any other un-skipped character in the output stream gives an error.
#[derive(Debug)]
pub struct NetpbmAscii1bppIter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAscii1bppIter<'b> {
  #[inline]
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAscii1bppIter<'b> {
  type Item = Result<bool, NetpbmError>;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    let b = self.spare.get(0)?;
    let out = Some(match b {
      b'0' => Ok(false),
      b'1' => Ok(true),
      _ => Err(NetpbmError::CouldNotParseUnsigned),
    });
    self.spare = netpbm_trim_comments_and_whitespace(&self.spare[1..]);
    out
  }
}

/// Parses u16 ascii entries.
#[derive(Debug)]
pub struct NetpbmAsciiU16Iter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAsciiU16Iter<'b> {
  #[inline]
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAsciiU16Iter<'b> {
  type Item = Result<u16, NetpbmError>;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      None
    } else {
      match netpbm_read_ascii_unsigned(self.spare) {
        Ok((u, rest)) => {
          self.spare = netpbm_trim_comments_and_whitespace(rest);
          if u <= (u16::MAX as u32) {
            Some(Ok(u as u16))
          } else {
            Some(Err(NetpbmError::IntegerExceedsMaxValue))
          }
        }
        Err(e) => Some(Err(e)),
      }
    }
  }
}

/// Parses u8 ascii entries.
#[derive(Debug)]
pub struct NetpbmAsciiU8Iter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAsciiU8Iter<'b> {
  #[inline]
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAsciiU8Iter<'b> {
  type Item = Result<u8, NetpbmError>;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      None
    } else {
      match netpbm_read_ascii_unsigned(self.spare) {
        Ok((u, rest)) => {
          self.spare = netpbm_trim_comments_and_whitespace(rest);
          if u <= (u8::MAX as u32) {
            Some(Ok(u as u8))
          } else {
            Some(Err(NetpbmError::IntegerExceedsMaxValue))
          }
        }
        Err(e) => Some(Err(e)),
      }
    }
  }
}

#[cfg(feature = "alloc")]
impl<P> crate::image::Bitmap<P>
where
  P: From<r8g8b8a8_Srgb> + Clone,
{
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn try_from_netpbm_bytes(netpbm: &[u8]) -> Result<Self, NetpbmError> {
    use alloc::vec::Vec;
    use bytemuck::*;
    use core::mem::size_of;
    use wide::*;
    //
    // /// Iterates every 1 bit of the byte, going from high to low.
    ///
    /// This returns all bits in the sequence, so use `take` as necessary.
    #[inline]
    pub fn iter_1bpp_high_to_low(bytes: &[u8]) -> impl Iterator<Item = bool> + '_ {
      bytes.iter().copied().flat_map(|byte| {
        [
          (byte & 0b1000_0000) != 0,
          (byte & 0b0100_0000) != 0,
          (byte & 0b0010_0000) != 0,
          (byte & 0b0001_0000) != 0,
          (byte & 0b0000_1000) != 0,
          (byte & 0b0000_0100) != 0,
          (byte & 0b0000_0010) != 0,
          (byte & 0b0000_0001) != 0,
        ]
        .into_iter()
      })
    }

    //
    let (header, pixel_data) = netpbm_parse_header(netpbm)?;
    let pixel_count = header.width.saturating_mul(header.height) as usize;
    let mut image: Vec<P> = Vec::new();
    image.try_reserve(pixel_count)?;
    //
    let black = r8g8b8a8_Srgb { r: 0, g: 0, b: 0, a: 0xFF };
    let white = r8g8b8a8_Srgb { r: 0xFF, g: 0xFF, b: 0xFF, a: 0xFF };
    match header.data_format {
      NetpbmDataFormat::Ascii_Y_1bpp => {
        NetpbmAscii1bppIter::new(pixel_data)
          .filter_map(|r| r.ok())
          .map(|b| if b { black } else { white })
          .take(pixel_count)
          .for_each(|color| image.push(color.into()));
      }
      NetpbmDataFormat::Ascii_Y_U8 { max: u8::MAX } => {
        // When values are the full u8 range, we don't need to re-scale.
        NetpbmAsciiU8Iter::new(pixel_data)
          .filter_map(|r| r.ok())
          .take(pixel_count)
          .for_each(|y| image.push(r8g8b8a8_Srgb { r: y, g: y, b: y, a: 255 }.into()));
      }
      NetpbmDataFormat::Ascii_Y_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let mut u8_it = NetpbmAsciiU8Iter::new(pixel_data).filter_map(|r| r.ok());
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = u8_it.next() {
          let one = u8_it.next().unwrap_or_default();
          let two = u8_it.next().unwrap_or_default();
          let three = u8_it.next().unwrap_or_default();
          let y_raw =
            i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Ascii_Y_U16 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let mut u16_it = NetpbmAsciiU16Iter::new(pixel_data).filter_map(|r| r.ok());
        // since we're processing 4 pixels at a time, we might have a partial batch at
        // the end.
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = u16_it.next() {
          let one = u16_it.next().unwrap_or_default();
          let two = u16_it.next().unwrap_or_default();
          let three = u16_it.next().unwrap_or_default();
          let y_raw =
            i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16_u32) | (y_scaled_u << 8_u32) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Ascii_RGB_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let mut u8_it =
          NetpbmAsciiU8Iter::new(pixel_data).filter_map(|r| r.ok()).take(3 * pixel_count);
        while let Some(r_raw) = u8_it.next() {
          let g_raw = u8_it.next().unwrap_or_default();
          let b_raw = u8_it.next().unwrap_or_default();
          let a_raw = 255_u8;
          let rgba_raw =
            i32x4::from([r_raw as i32, g_raw as i32, b_raw as i32, a_raw as i32]).round_float();
          let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
          let [r32, g32, b32, a32]: [u32; 4] = cast(rgba_scaled_f.round_int());
          image
            .push(r8g8b8a8_Srgb { r: r32 as u8, g: g32 as u8, b: b32 as u8, a: a32 as u8 }.into());
        }
      }
      NetpbmDataFormat::Ascii_RGB_U16 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let mut u16_it =
          NetpbmAsciiU16Iter::new(pixel_data).filter_map(|r| r.ok()).take(3 * pixel_count);
        while let Some(r_raw) = u16_it.next() {
          let g_raw = u16_it.next().unwrap_or_default();
          let b_raw = u16_it.next().unwrap_or_default();
          let a_raw = 255_u8;
          let rgba_raw =
            i32x4::from([r_raw as i32, g_raw as i32, b_raw as i32, a_raw as i32]).round_float();
          let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
          let [r32, g32, b32, a32]: [u32; 4] = cast(rgba_scaled_f.round_int());
          image
            .push(r8g8b8a8_Srgb { r: r32 as u8, g: g32 as u8, b: b32 as u8, a: a32 as u8 }.into());
        }
      }
      NetpbmDataFormat::Binary_Y_1bpp => {
        let black = r8g8b8a8_Srgb { r: 0, g: 0, b: 0, a: 0xFF };
        let white = r8g8b8a8_Srgb { r: 0xFF, g: 0xFF, b: 0xFF, a: 0xFF };
        iter_1bpp_high_to_low(pixel_data)
          .map(|b| if b { black } else { white })
          .take(pixel_count)
          .for_each(|color| image.push(color.into()));
      }
      NetpbmDataFormat::Binary_Y_U8 { max: u8::MAX } => {
        pixel_data
          .iter()
          .copied()
          .take(pixel_count)
          .for_each(|y| image.push(r8g8b8a8_Srgb { r: y, g: y, b: y, a: 255 }.into()));
      }
      NetpbmDataFormat::Binary_Y_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let mut u8_it = pixel_data.iter().copied().take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = u8_it.next() {
          let one = u8_it.next().unwrap_or_default();
          let two = u8_it.next().unwrap_or_default();
          let three = u8_it.next().unwrap_or_default();
          let y_raw =
            i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Binary_Y_U16BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let mut u16_it = pixel_data_u16be.iter().copied().map(u16::from_be_bytes).take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = u16_it.next() {
          let one = u16_it.next().unwrap_or_default();
          let two = u16_it.next().unwrap_or_default();
          let three = u16_it.next().unwrap_or_default();
          let y_raw =
            i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Binary_Y_F32BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = f32x4::from(max);
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let pixel_data_u16be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let mut f32_it = pixel_data_u16be.iter().copied().map(f32::from_be_bytes).take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = f32_it.next() {
          let one = f32_it.next().unwrap_or_default();
          let two = f32_it.next().unwrap_or_default();
          let three = f32_it.next().unwrap_or_default();
          let y_raw = f32x4::from([zero, one, two, three]);
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Binary_Y_F32LE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = f32x4::from(max);
        let a_x4 = u32x4::from(u8::MAX as u32) << 24;
        let pixel_data_u16be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let mut f32_it = pixel_data_u16be.iter().copied().map(f32::from_le_bytes).take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some(zero) = f32_it.next() {
          let one = f32_it.next().unwrap_or_default();
          let two = f32_it.next().unwrap_or_default();
          let three = f32_it.next().unwrap_or_default();
          let y_raw = f32x4::from([zero, one, two, three]);
          let y_scaled_f = (y_raw / image_max) * channel_max;
          let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
          // little endian, so bytes are packed into lanes as BGRA
          let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
          let rgba_array: [r8g8b8a8_Srgb; 4] = cast(rgba_x4);
          rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p.into()));
          pixels_remaining = pixels_remaining.saturating_sub(4);
        }
      }
      NetpbmDataFormat::Binary_YA_U8 { max: u8::MAX } => {
        let pixel_data_ya: &[[u8; 2]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        pixel_data_ya
          .iter()
          .copied()
          .take(pixel_count)
          .for_each(|[y, a]| image.push(r8g8b8a8_Srgb { r: y, g: y, b: y, a }.into()));
      }
      NetpbmDataFormat::Binary_YA_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_ya: &[[u8; 2]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let mut ya_it = pixel_data_ya.iter().copied().take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some([y0, a0]) = ya_it.next() {
          let [y1, a1] = ya_it.next().unwrap_or_default();
          let ya_raw = i32x4::from([y0 as i32, a0 as i32, y1 as i32, a1 as i32]).round_float();
          let ya_scaled_f = (ya_raw / image_max) * channel_max;
          let [y0, a0, y1, a1]: [u32; 4] = cast(ya_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: y0 as u8, g: y0 as u8, b: y0 as u8, a: a0 as u8 }.into());
          if pixels_remaining >= 2 {
            image.push(r8g8b8a8_Srgb { r: y1 as u8, g: y1 as u8, b: y1 as u8, a: a1 as u8 }.into());
          }
          pixels_remaining = pixels_remaining.saturating_sub(2);
        }
      }
      NetpbmDataFormat::Binary_YA_U16BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_ya: &[[u8; 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let mut ya_it = pixel_data_ya
          .iter()
          .copied()
          .map(|arr| {
            [
              u16::from_be_bytes(arr[0..2].try_into().unwrap()),
              u16::from_be_bytes(arr[2..4].try_into().unwrap()),
            ]
          })
          .take(pixel_count);
        let mut pixels_remaining = pixel_count;
        while let Some([y0, a0]) = ya_it.next() {
          let [y1, a1] = ya_it.next().unwrap_or_default();
          let ya_raw = i32x4::from([y0 as i32, a0 as i32, y1 as i32, a1 as i32]).round_float();
          let ya_scaled_f = (ya_raw / image_max) * channel_max;
          let [y0, a0, y1, a1]: [u32; 4] = cast(ya_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: y0 as u8, g: y0 as u8, b: y0 as u8, a: a0 as u8 }.into());
          if pixels_remaining >= 2 {
            image.push(r8g8b8a8_Srgb { r: y1 as u8, g: y1 as u8, b: y1 as u8, a: a1 as u8 }.into());
          }
          pixels_remaining = pixels_remaining.saturating_sub(2);
        }
      }
      NetpbmDataFormat::Binary_RGB_U8 { max: u8::MAX } => {
        let pixel_data_ya: &[[u8; 3]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        pixel_data_ya
          .iter()
          .copied()
          .take(pixel_count)
          .for_each(|[r, g, b]| image.push(r8g8b8a8_Srgb { r, g, b, a: u8::MAX }.into()));
      }
      NetpbmDataFormat::Binary_RGB_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; 3]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
        rgb_it.for_each(|[r, g, b]| {
          let rgb_raw = i32x4::from([r as i32, g as i32, b as i32, max as i32]).round_float();
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGB_U16BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<u16>() * 3]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
        rgb_it.for_each(|[r0, r1, g0, g1, b0, b1]| {
          let rgb_raw = i32x4::from([
            u16::from_be_bytes([r0, r1]) as i32,
            u16::from_be_bytes([g0, g1]) as i32,
            u16::from_be_bytes([b0, b1]) as i32,
            max as i32,
          ])
          .round_float();
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGB_F32BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<f32>() * 3]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgb_it = pixel_data_rgb
          .iter()
          .copied()
          .map(|a| {
            [
              f32::from_be_bytes(a[0..4].try_into().unwrap()),
              f32::from_be_bytes(a[4..8].try_into().unwrap()),
              f32::from_be_bytes(a[8..12].try_into().unwrap()),
            ]
          })
          .take(pixel_count);
        rgb_it.for_each(|[r, g, b]| {
          let rgb_raw = f32x4::from([r, g, b, max]);
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGB_F32LE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<f32>() * 3]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgb_it = pixel_data_rgb
          .iter()
          .copied()
          .map(|a| {
            [
              f32::from_le_bytes(a[0..4].try_into().unwrap()),
              f32::from_le_bytes(a[4..8].try_into().unwrap()),
              f32::from_le_bytes(a[8..12].try_into().unwrap()),
            ]
          })
          .take(pixel_count);
        rgb_it.for_each(|[r, g, b]| {
          let rgb_raw = f32x4::from([r, g, b, max]);
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGBA_U8 { max: u8::MAX } => {
        let pixel_data_ya: &[[u8; 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        pixel_data_ya
          .iter()
          .copied()
          .take(pixel_count)
          .for_each(|[r, g, b, a]| image.push(r8g8b8a8_Srgb { r, g, b, a }.into()));
      }
      NetpbmDataFormat::Binary_RGBA_U8 { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgba_it = pixel_data_rgb.iter().copied().take(pixel_count);
        rgba_it.for_each(|[r, g, b, a]| {
          let rgb_raw = i32x4::from([r as i32, g as i32, b as i32, a as i32]).round_float();
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGBA_U16BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<u16>() * 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
        rgb_it.for_each(|[r0, r1, g0, g1, b0, b1, a0, a1]| {
          let rgb_raw = i32x4::from([
            u16::from_be_bytes([r0, r1]) as i32,
            u16::from_be_bytes([g0, g1]) as i32,
            u16::from_be_bytes([b0, b1]) as i32,
            u16::from_be_bytes([a0, a1]) as i32,
          ])
          .round_float();
          let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGBA_F32BE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<f32>() * 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgba_it = pixel_data_rgb
          .iter()
          .copied()
          .map(|a| {
            [
              f32::from_be_bytes(a[0..4].try_into().unwrap()),
              f32::from_be_bytes(a[4..8].try_into().unwrap()),
              f32::from_be_bytes(a[8..12].try_into().unwrap()),
              f32::from_be_bytes(a[12..16].try_into().unwrap()),
            ]
          })
          .take(pixel_count);
        rgba_it.for_each(|[r, g, b, a]| {
          let rgba_raw = f32x4::from([r, g, b, a]);
          let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgba_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
      NetpbmDataFormat::Binary_RGBA_F32LE { max } => {
        let channel_max = i32x4::from(u8::MAX as i32).round_float();
        let image_max = i32x4::from(max as i32).round_float();
        let pixel_data_rgb: &[[u8; size_of::<f32>() * 4]] = match try_cast_slice(pixel_data) {
          Ok(s) => s,
          Err(_) => return Err(NetpbmError::InsufficientBytes),
        };
        let rgba_it = pixel_data_rgb
          .iter()
          .copied()
          .map(|a| {
            [
              f32::from_le_bytes(a[0..4].try_into().unwrap()),
              f32::from_le_bytes(a[4..8].try_into().unwrap()),
              f32::from_le_bytes(a[8..12].try_into().unwrap()),
              f32::from_le_bytes(a[12..16].try_into().unwrap()),
            ]
          })
          .take(pixel_count);
        rgba_it.for_each(|[r, g, b, a]| {
          let rgba_raw = f32x4::from([r, g, b, a]);
          let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
          let [r, g, b, a]: [u32; 4] = cast(rgba_scaled_f.round_int());
          image.push(r8g8b8a8_Srgb { r: r as u8, g: g as u8, b: b as u8, a: a as u8 }.into());
        });
      }
    }
    if image.len() < pixel_count {
      let black = r8g8b8a8_Srgb { r: 0, g: 0, b: 0, a: 0xFF };
      image.resize(pixel_count, black.into());
    }
    let bitmap = Bitmap { pixels: image, width: header.width, height: header.height };
    Ok(bitmap)
  }
}
