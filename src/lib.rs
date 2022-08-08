#![no_std]
#![allow(unused_imports)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

//! A crate to work with image data.

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod basic_image;
#[cfg(feature = "alloc")]
pub use basic_image::*;

#[cfg(feature = "png")]
#[cfg_attr(docs_rs, doc(cfg(feature = "png")))]
pub mod png;
