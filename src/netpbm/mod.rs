#![forbid(unsafe_code)]

//! This module gives support for the various
//! [Netpbm](https://en.wikipedia.org/wiki/Netpbm) formats.
//!
//! Several file extensions are used by this format family: `.pbm`, `.pgm`,
//! `.ppm`, `.pnm`, and `.pam`. They're all extremely formats with absolutely no
//! compression.
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
//! TODO
//!
//! ## `Pf`, `PF`, and `PF4`
//!
//! TODO
//!
//! ## `P7`
//!
//! TODO
