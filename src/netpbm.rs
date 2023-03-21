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

use core::{
  num::ParseIntError,
  str::{from_utf8, Utf8Error},
};

use pixel_formats::r32g32b32_Sfloat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetpbmError {
  #[cfg(feature = "alloc")]
  AllocError,
  ParseError,
  /// The tag value given wasn't in the supported range of `1..=6`
  TagError,
  /// The current version doesn't handle maximum values above 255
  MaxValueError,
}
impl From<Utf8Error> for NetpbmError {
  #[inline]
  fn from(_: Utf8Error) -> Self {
    NetpbmError::ParseError
  }
}
impl From<ParseIntError> for NetpbmError {
  #[inline]
  fn from(_: ParseIntError) -> Self {
    NetpbmError::ParseError
  }
}
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for NetpbmError {
  #[inline]
  fn from(_: alloc::collections::TryReserveError) -> Self {
    NetpbmError::AllocError
  }
}

#[derive(Debug, Clone, Copy)]
pub struct NetpbmHeader {
  /// The tag sets the format of the bytes after the header:
  /// * 1: ascii 1-bit
  /// * 2: ascii grayscale
  /// * 3: ascii rgb
  /// * 4: binary 1-bit
  /// * 5: binary grayscale
  /// * 6: binary rgb
  pub tag: u8,
  pub width: u32,
  pub height: u32,
  /// Max value per channel entry.
  pub max: u32,
}

#[inline]
pub fn netpbm_trim(mut bytes: &[u8]) -> &[u8] {
  'trim: loop {
    match bytes {
      // trim leading whitespace
      [u, tail @ ..] if u.is_ascii_whitespace() => bytes = tail,

      // trim single-line comment
      [b'#', tail @ ..] => {
        let mut it = tail.splitn(2, |&u| u == b'\n');
        drop(it.next());
        bytes = it.next().unwrap_or(&[]);
      }

      // now we're done
      _ => return bytes,
    }
  }
}
#[inline]
pub fn netpbm_pull_tag(bytes: &[u8]) -> Result<(u8, &[u8]), NetpbmError> {
  match bytes {
    [b'P', tag, rest @ ..] => Ok((tag.wrapping_sub(b'0'), netpbm_trim(rest))),
    _ => Err(NetpbmError::ParseError),
  }
}
#[inline]
pub fn netpbm_pull_ascii_u32(bytes: &[u8]) -> Result<(u32, &[u8]), NetpbmError> {
  let mut it = bytes.splitn(2, |u| !u.is_ascii_digit());
  let digits = it.next().ok_or(NetpbmError::ParseError)?;
  let spare = it.next().ok_or(NetpbmError::ParseError)?;
  let digits_str = from_utf8(digits)?;
  let number = digits_str.parse::<u32>()?;
  Ok((number, netpbm_trim(spare)))
}
#[inline]
pub fn netpbm_pull_header(bytes: &[u8]) -> Result<(NetpbmHeader, &[u8]), NetpbmError> {
  let (tag, rest) = netpbm_pull_tag(bytes)?;
  if !(1..=6).contains(&tag) {
    return Err(NetpbmError::TagError);
  }
  let (width, rest) = netpbm_pull_ascii_u32(rest)?;
  let (height, rest) = netpbm_pull_ascii_u32(rest)?;
  match tag {
    1 | 4 => Ok((NetpbmHeader { tag, width, height, max: 1 }, rest)),
    2 | 3 | 5 | 6 => {
      let (max, rest) = netpbm_pull_ascii_u32(rest)?;
      Ok((NetpbmHeader { tag, width, height, max }, rest))
    }
    _ => unreachable!(),
  }
}

#[inline]
pub fn netpbm_iter_p1(mut bytes: &[u8]) -> impl Iterator<Item = bool> + '_ {
  core::iter::from_fn(move || {
    let (out, tail) = match bytes {
      [b'0', tail @ ..] => (false, tail),
      [b'1', tail @ ..] => (true, tail),
      _ => return None,
    };
    bytes = netpbm_trim(tail);
    Some(out)
  })
}
#[inline]
pub fn netpbm_iter_p2(mut bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
  core::iter::from_fn(move || {
    let (out, tail) = netpbm_pull_ascii_u32(bytes).ok()?;
    bytes = netpbm_trim(tail);
    Some(out)
  })
}
#[inline]
pub fn netpbm_iter_p3(mut bytes: &[u8]) -> impl Iterator<Item = [u32; 3]> + '_ {
  core::iter::from_fn(move || {
    let (r, tail) = netpbm_pull_ascii_u32(bytes).ok()?;
    let (g, tail) = netpbm_pull_ascii_u32(netpbm_trim(tail)).ok()?;
    let (b, tail) = netpbm_pull_ascii_u32(netpbm_trim(tail)).ok()?;
    bytes = netpbm_trim(tail);
    Some([r, g, b])
  })
}
#[inline]
pub fn netpbm_iter_p4(bytes: &[u8]) -> impl Iterator<Item = bool> + '_ {
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
#[inline]
pub fn netpbm_iter_p5(bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
  bytes.iter().copied()
}
#[inline]
pub fn netpbm_iter_p6(mut bytes: &[u8]) -> impl Iterator<Item = [u8; 3]> + '_ {
  core::iter::from_fn(move || {
    let (out, tail): ([u8; 3], &[u8]) = match bytes {
      [r, g, b, tail @ ..] => ([*r, *g, *b], tail),
      [r, g] => ([*r, *g, 0], &[]),
      [r] => ([*r, 0, 0], &[]),
      [] => return None,
    };
    bytes = tail;
    Some(out)
  })
}

#[inline]
pub fn netpbm_for_each_rgb<F: FnMut(r32g32b32_Sfloat)>(bytes: &[u8], f: F) {
  if let Ok((header, rest)) = netpbm_pull_header(bytes) {
    match header.tag {
      1 => netpbm_iter_p1(rest)
        .map(|b| {
          if b {
            r32g32b32_Sfloat { r: 0.0, g: 0.0, b: 0.0 }
          } else {
            r32g32b32_Sfloat { r: 1.0, g: 1.0, b: 1.0 }
          }
        })
        .for_each(f),
      2 => netpbm_iter_p2(rest)
        .map(|y| {
          let yf = (y as f32) / (header.max as f32);
          r32g32b32_Sfloat { r: yf, g: yf, b: yf }
        })
        .for_each(f),
      3 => netpbm_iter_p3(rest)
        .map(|[r, g, b]| {
          let rf = (r as f32) / (header.max as f32);
          let gf = (g as f32) / (header.max as f32);
          let bf = (b as f32) / (header.max as f32);
          r32g32b32_Sfloat { r: rf, g: gf, b: bf }
        })
        .for_each(f),
      4 => netpbm_iter_p4(rest)
        .map(|b| {
          if b {
            r32g32b32_Sfloat { r: 0.0, g: 0.0, b: 0.0 }
          } else {
            r32g32b32_Sfloat { r: 1.0, g: 1.0, b: 1.0 }
          }
        })
        .for_each(f),
      5 => netpbm_iter_p5(rest)
        .map(|y| {
          let yf = (y as f32) / (header.max as f32);
          r32g32b32_Sfloat { r: yf, g: yf, b: yf }
        })
        .for_each(f),
      6 => netpbm_iter_p6(rest)
        .map(|[r, g, b]| {
          let rf = (r as f32) / (header.max as f32);
          let gf = (g as f32) / (header.max as f32);
          let bf = (b as f32) / (header.max as f32);
          r32g32b32_Sfloat { r: rf, g: gf, b: bf }
        })
        .for_each(f),
      _ => unimplemented!(),
    }
  }
}

#[cfg(feature = "alloc")]
#[inline]
pub fn netpbm_try_bitmap<P>(bytes: &[u8]) -> Result<crate::image::Bitmap<P>, NetpbmError>
where
  P: From<r32g32b32_Sfloat>,
{
  use alloc::vec::Vec;
  //
  let (header, _rest) = netpbm_pull_header(bytes)?;
  let mut pixels: Vec<P> = {
    let mut v = Vec::new();
    v.try_reserve(header.width.saturating_mul(header.height) as usize)?;
    v
  };
  netpbm_for_each_rgb(bytes, |p| pixels.push(p.into()));
  Ok(crate::image::Bitmap { width: header.width, height: header.height, pixels })
}
