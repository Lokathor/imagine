#![forbid(unsafe_code)]

//! The various [Netpbm](https://en.wikipedia.org/wiki/Netpbm) formats.
//!
//! This supports the `P1` through `P6` formats:
//! * `P2` and `P3` can have any maximum that fits in `u32`.
//! * `P5` and `P6` can have any maximum that fits in `u8`.
//!
//! Generally, you should just use the [`netpbm_try_bitmap`] function to
//! generate a [Bitmap](crate::image::Bitmap) from the RGB data with a single
//! function call (requires the `alloc` crate feature).

use core::{
  num::ParseIntError,
  str::{from_utf8, Utf8Error},
};
use pixel_formats::*;

use crate::ImagineError;

/// Header info for a Netpbm file.
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
  /// Image width
  pub width: u32,
  /// Image height
  pub height: u32,
  /// Max value per channel entry.
  pub max: u32,
}

/// Pulls the tag off the front of the bytes
#[inline]
#[doc(hidden)]
pub const fn netpbm_pull_tag(bytes: &[u8]) -> Result<(u8, &[u8]), ImagineError> {
  match bytes {
    [b'P', tag, rest @ ..] => Ok((tag.wrapping_sub(b'0'), rest)),
    _ => Err(ImagineError::Parse),
  }
}

/// Trims to a `'\n'`, but not past that.
fn trim_to_eol(bytes: &[u8]) -> &[u8] {
  let mut it = bytes.splitn(2, |&u| u == b'\n');
  drop(it.next());
  it.next().unwrap_or(&[])
}

/// Trims leading whitespace and comments from the bytes
#[inline]
#[doc(hidden)]
pub fn netpbm_trim(mut bytes: &[u8]) -> &[u8] {
  'trim: loop {
    match bytes {
      // trim leading whitespace
      [u, tail @ ..] if u.is_ascii_whitespace() => bytes = tail,

      // trim single-line comment
      [b'#', tail @ ..] => bytes = trim_to_eol(tail),

      // now we're done
      _ => return bytes,
    }
  }
}

/// Pulls an ascii u32 value off the front of the bytes
#[inline]
#[doc(hidden)]
pub fn netpbm_pull_ascii_u32(bytes: &[u8]) -> Result<(u32, &[u8]), ImagineError> {
  let mut it = bytes.splitn(2, |u| !u.is_ascii_digit());
  let digits = it.next().ok_or(ImagineError::Parse)?;
  let spare = it.next().ok_or(ImagineError::Parse)?;
  let digits_str = from_utf8(digits)?;
  let number = digits_str.parse::<u32>()?;
  Ok((number, spare))
}

/// Get the header from the Netpbm bytes, as well as the rest of the data.
#[inline]
pub fn netpbm_pull_header(bytes: &[u8]) -> Result<(NetpbmHeader, &[u8]), ImagineError> {
  let (tag, rest) = netpbm_pull_tag(bytes)?;
  if !(1..=6).contains(&tag) {
    return Err(ImagineError::Parse);
  }
  let (width, rest) = netpbm_pull_ascii_u32(netpbm_trim(rest))?;
  let (height, rest) = netpbm_pull_ascii_u32(netpbm_trim(rest))?;
  Ok(match tag {
    // ascii paths use a full trim
    1 => (NetpbmHeader { tag, width, height, max: 1 }, netpbm_trim(rest)),
    2 | 3 => {
      let (max, rest) = netpbm_pull_ascii_u32(netpbm_trim(rest))?;
      (NetpbmHeader { tag, width, height, max }, netpbm_trim(rest))
    }
    // binary paths must only trim to the end of the current line
    4 => (NetpbmHeader { tag, width, height, max: 1 }, trim_to_eol(rest)),
    5 | 6 => {
      let (max, rest) = netpbm_pull_ascii_u32(rest)?;
      (NetpbmHeader { tag, width, height, max }, trim_to_eol(rest))
    }
    _ => unreachable!(),
  })
}

/// Iterate post-header P1 data.
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
/// Iterate post-header P2 data.
#[inline]
pub fn netpbm_iter_p2(mut bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
  core::iter::from_fn(move || {
    let (out, tail) = netpbm_pull_ascii_u32(bytes).ok()?;
    bytes = netpbm_trim(tail);
    Some(out)
  })
}
/// Iterate post-header P3 data.
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
/// Iterate post-header P4 data.
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
/// Iterate post-header P5 data.
#[inline]
pub fn netpbm_iter_p5(bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
  bytes.iter().copied()
}
/// Iterate post-header P6 data.
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

/// Parse the file bytes for a header and then run the `f` given for each pixel.
///
/// Pixels will be produced left to right, top to bottom.
///
/// This iterator will automatically limit itself to processing *at most* the
/// `width` and `height` found in the header. If there's more data than that it
/// will be ignored.
#[inline]
pub fn netpbm_for_each_rgb<F: FnMut(r32g32b32_Sfloat)>(
  bytes: &[u8], f: F,
) -> Result<(), ImagineError> {
  let (header, rest) = netpbm_pull_header(bytes)?;
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  match header.tag {
    1 => netpbm_iter_p1(rest)
      .take(target_pixel_count)
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
      .take(target_pixel_count)
      .map(|[r, g, b]| {
        let rf = (r as f32) / (header.max as f32);
        let gf = (g as f32) / (header.max as f32);
        let bf = (b as f32) / (header.max as f32);
        r32g32b32_Sfloat { r: rf, g: gf, b: bf }
      })
      .for_each(f),
    4 => netpbm_iter_p4(rest)
      .take(target_pixel_count)
      .map(|b| {
        if b {
          r32g32b32_Sfloat { r: 0.0, g: 0.0, b: 0.0 }
        } else {
          r32g32b32_Sfloat { r: 1.0, g: 1.0, b: 1.0 }
        }
      })
      .for_each(f),
    5 => netpbm_iter_p5(rest)
      .take(target_pixel_count)
      .map(|y| {
        let yf = (y as f32) / (header.max as f32);
        r32g32b32_Sfloat { r: yf, g: yf, b: yf }
      })
      .for_each(f),
    6 => netpbm_iter_p6(rest)
      .take(target_pixel_count)
      .map(|[r, g, b]| {
        let rf = (r as f32) / (header.max as f32);
        let gf = (g as f32) / (header.max as f32);
        let bf = (b as f32) / (header.max as f32);
        r32g32b32_Sfloat { r: rf, g: gf, b: bf }
      })
      .for_each(f),
    _ => return Err(ImagineError::IncompleteLibrary),
  }
  Ok(())
}

/// Automatically allocate and fill in a [Bitmap](crate::image::Bitmap).
///
/// If the file has less than `width * height` pixels defined, the remainder
/// will be filled with black. If *more* pixels than that are defined the excess
/// data will be ignored.
///
/// Per the file format's definition, the origin of the image is always the top
/// left.
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn netpbm_try_bitmap_rgb<P>(bytes: &[u8]) -> Result<crate::bitmap::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32_Sfloat>,
{
  use alloc::vec::Vec;
  //
  let (header, _rest) = netpbm_pull_header(bytes)?;
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut pixels: Vec<P> = {
    let mut v = Vec::new();
    v.try_reserve(target_pixel_count)?;
    v
  };
  netpbm_for_each_rgb(bytes, |p| pixels.push(p.into()))?;
  let black: P = P::from(r32g32b32_Sfloat::BLACK);
  pixels.resize(target_pixel_count, black);
  Ok(crate::bitmap::Bitmap { width: header.width, height: header.height, pixels })
}

/// Automatically allocate and fill in a [Bitmap](crate::image::Bitmap).
///
/// This works just like [`netpbm_try_bitmap_rgb`], but automatically adds an
/// alpha value (full opacity).
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn netpbm_try_bitmap_rgba<P>(bytes: &[u8]) -> Result<crate::bitmap::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32a32_Sfloat>,
{
  use alloc::vec::Vec;
  //
  let (header, _rest) = netpbm_pull_header(bytes)?;
  if header.width > 17_000 || header.height > 17_000 {
    return Err(ImagineError::DimensionsTooLarge);
  }
  let target_pixel_count: usize =
    header.width.checked_mul(header.height).ok_or(ImagineError::Value)?.try_into().unwrap();
  let mut pixels: Vec<P> = {
    let mut v = Vec::new();
    v.try_reserve(target_pixel_count)?;
    v
  };
  netpbm_for_each_rgb(bytes, |p| pixels.push(P::from(r32g32b32a32_Sfloat::from(p))))?;
  let black: P = P::from(r32g32b32a32_Sfloat::OPAQUE_BLACK);
  pixels.resize(target_pixel_count, black);
  Ok(crate::bitmap::Bitmap { width: header.width, height: header.height, pixels })
}
