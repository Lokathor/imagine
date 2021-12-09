#![forbid(unsafe_code)]

//! This module gives support for the various
//! [Netpbm](https://en.wikipedia.org/wiki/Netpbm) formats.
//!
//! The general idea of these formats is that there's an ascii header which
//! describes the image basics, followed by either ascii or binary data giving
//! the value of every single pixel in the image.
//! * Comments are marked with `#` and go to the end of the line (like TOML).
//! * Whitespace is generally insignificant.
//! * Multi-byte binary values are generally big-endian.
//! * Pixel flow is generally left to right, top to bottom.
//!
//! Because the format is so simple most of the parsing just requires calling a
//! few of Rust's various [Iterator](core::iter::Iterator) methods.
