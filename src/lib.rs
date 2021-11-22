#![no_std]
#![warn(missing_docs)]
#![allow(unused_imports)]

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

#[cfg(feature = "png")]
pub mod png;
