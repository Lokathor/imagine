#![no_std]
#![allow(unused_imports)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(target_pointer_width = "16")]
compile_error!("this crate assumes 32-bit or bigger pointers!");

pub mod pixel_formats;

pub mod png;
