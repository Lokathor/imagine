#![no_std]
#![cfg_attr(docs_rs, feature(doc_cfg))]
#![allow(unused_imports)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(clippy::missing_inline_in_public_items)]
#![allow(clippy::get_first)]

//! A crate to work with image data.

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
#[allow(missing_docs)]
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
