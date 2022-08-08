#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod basic_image;
#[cfg(feature = "alloc")]
pub use basic_image::*;

#[cfg(feature = "png")]
pub mod png;
