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

pub struct P1Header {
  pub width: u32,
  pub height: u32,
}

pub struct P2Header {
  pub width: u32,
  pub height: u32,
  pub max_value: u16,
}

pub struct P3Header {
  pub width: u32,
  pub height: u32,
  pub max_value: u16,
}

pub struct P4Header {
  pub width: u32,
  pub height: u32,
}

pub struct P5Header {
  pub width: u32,
  pub height: u32,
  pub max_value: u16,
}

pub struct P6Header {
  pub width: u32,
  pub height: u32,
  pub max_value: u16,
}

pub struct PFHeader {
  pub width: u32,
  pub height: u32,
  pub max_value: f32,
}

pub struct PfHeader {
  pub width: u32,
  pub height: u32,
  pub max_value: f32,
}

pub enum P7Channels {
  Y,
  YA,
  RGB,
  RGBA,
}

pub struct P7Header {
  pub width: u32,
  pub height: u32,
  pub max_value: u16,
  pub channels: P7Channels,
}
