#![no_std]
#![cfg_attr(docs_rs, feature(doc_cfg))]
#![allow(unused_imports)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![allow(clippy::get_first)]

//! A crate to work with image data.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod pixel_formats;

#[cfg(feature = "alloc")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
pub mod image;

#[cfg(feature = "png")]
#[cfg_attr(docs_rs, doc(cfg(feature = "png")))]
pub mod png;

#[cfg(feature = "bmp")]
#[cfg_attr(docs_rs, doc(cfg(feature = "bmp")))]
pub mod bmp;
