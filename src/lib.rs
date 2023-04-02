#![no_std]
#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![allow(unused_labels)]
//#![allow(unused_imports)]
#![allow(clippy::drop_copy)]
#![allow(clippy::get_first)]
#![allow(non_upper_case_globals)]
#![allow(clippy::upper_case_acronyms)]
#![cfg_attr(docs_rs, feature(doc_cfg))]
#![warn(missing_debug_implementations)]
#![warn(clippy::missing_inline_in_public_items)]

//! A crate to work with image data.

#[cfg(feature = "alloc")]
extern crate alloc;

use pixel_formats::*;

mod ascii_array;
mod error;
mod util;

mod borrowed_bitmap;
pub use self::{borrowed_bitmap::BorrowedBitmap, error::*};

#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
mod alloc_bitmap;
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub use self::alloc_bitmap::{Bitmap, Palmap};

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

/// Automatically allocate and fill in a [Bitmap](crate::Bitmap).
///
/// This will try every format compiled into the library until one of them
/// works, or will return a parse error if no format works. The order of trying
/// each format is unspecified, but that basically doesn't matter because you
/// can't really have a file that successfully parses as more than one format at
/// the same time.
///
/// The output image will automatically be vertically flipped as necessary to
/// respect the `origin_top_left` value given.
///
/// The output image data will use pre-multiplied alpha.
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub fn try_bitmap_rgba<P>(
  bytes: &[u8], origin_top_left: bool,
) -> Result<crate::Bitmap<P>, ImagineError>
where
  P: Copy + From<r32g32b32a32_Sfloat>,
{
  #[cfg(feature = "png")]
  if let Ok(bitmap) = png::png_try_bitmap_rgba(bytes, origin_top_left) {
    return Ok(bitmap);
  }
  #[cfg(feature = "bmp")]
  if let Ok(bitmap) = bmp::bmp_try_bitmap_rgba(bytes, origin_top_left) {
    return Ok(bitmap);
  }
  #[cfg(feature = "netpbm")]
  if let Ok(bitmap) = netpbm::netpbm_try_bitmap_rgba(bytes, origin_top_left) {
    return Ok(bitmap);
  }
  Err(ImagineError::Parse)
}
