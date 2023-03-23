#![no_std]
#![warn(missing_docs)]
#![allow(unused_labels)]
#![allow(unused_imports)]
#![allow(clippy::drop_copy)]
#![allow(clippy::get_first)]
#![allow(clippy::upper_case_acronyms)]
#![cfg_attr(docs_rs, feature(doc_cfg))]
#![warn(missing_debug_implementations)]
#![warn(clippy::missing_inline_in_public_items)]

//! A crate to work with image data.

mod ascii_array;
mod util;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub mod image;

#[cfg(feature = "png")]
#[cfg_attr(docs_rs, doc(cfg(feature = "png")))]
pub mod png;

#[cfg(feature = "bmp")]
#[cfg_attr(docs_rs, doc(cfg(feature = "bmp")))]
pub mod bmp;

#[cfg(feature = "netpbm")]
#[cfg_attr(docs_rs, doc(cfg(feature = "netpbm")))]
pub mod netpbm;

/// sRGB Intent for an image.
///
/// Unless you're able to color correct, the exact value doesn't really matter.
/// However, knowing that image data is sRGB or not *at all* can be helpful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
#[allow(missing_docs)]
pub enum sRGBIntent {
  Perceptual,
  RelativeColorimetric,
  Saturation,
  AbsoluteColorimetric,
}
