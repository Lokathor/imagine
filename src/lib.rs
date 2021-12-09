#![no_std]
#![cfg_attr(docs_rs, feature(doc_cfg))]
//#![warn(missing_docs)]
#![allow(unused_imports)]
//
#![allow(unused)]

//! A crate for image data decoding.
//!
//! Currently developing PNG support. In the future other image formats might
//! also be added.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(target_pointer_width = "16")]
compile_error!("this crate assumes 32-bit or bigger pointers!");

pub mod pixel_formats;
pub use pixel_formats::*;

pub mod ascii_array;
pub use ascii_array::*;

pub mod bit_depth_changes;
pub use bit_depth_changes::*;

mod parser_helpers;
pub(crate) use parser_helpers::*;

#[cfg(feature = "png")]
pub mod png;

#[cfg(feature = "bmp")]
pub mod bmp;

#[cfg(feature = "pbm")]
pub mod pbm;

/// Used by various image formats that support sRGB colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
// Note(Lokathor): This doesn't have direct impls to parse to and from bytes
// because each format uses different bytes to mean each of these options.
pub enum SrgbIntent {
  /// for images preferring good adaptation to the output device gamut at the
  /// expense of colorimetric accuracy, such as photographs.
  Perceptual,
  /// for images requiring colour appearance matching (relative to the output
  /// device white point), such as logos.
  RelativeColorimetric,
  /// for images preferring preservation of saturation at the expense of hue and
  /// lightness, such as charts and graphs.
  Saturation,
  /// for images requiring preservation of absolute colorimetry, such as
  /// previews of images destined for a different output device (proofs).
  AbsoluteColorimetric,
}
