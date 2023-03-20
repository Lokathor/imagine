#![forbid(unsafe_code)]
#![allow(nonstandard_style)]
#![allow(missing_docs)]

//! Module for working with PNG data.
//!
//! * [Portable Network Graphics (PNG) Specification (Third Edition)][png-spec]
//!
//! [png-spec]: https://www.w3.org/TR/png/
//!
//! ## Library Design Assumptions
//!
//! This library *can* run in a `no_alloc` scenario, using only user-provided
//! slices for each stage of the decoding. However, it still makes two major
//! assumption:
//!
//! * The entire PNG encoded source data stream is a single byte slice.
//! * Each stage of the decoding process goes into a single output buffer which
//!   must be large enough to fit all of the output of that stage at once.
//!
//! This library does *not* attempt to support "stream" decoding of PNG data,
//! keeping only a minimal amount of live data. It might be possible to create
//! such a thing using the types provided in this module, but that's not an
//! intended use case.
//!
//! ## Automatic Decoding
//!
//! Just call [`try_from_png_bytes`](crate::ImageRGBA8::try_from_png_bytes)
//! and the decoder will do its best.
//!
//! This requires the `alloc` and `miniz_oxide` crate features.
//!
//! ## Manual Decoding
//!
//! If you want full control over when allocations happen you can do that:
//!
//! 1) Call [`png_get_header`](png_get_header) to get the [`IHDR`] information
//!    for the PNG. This describes the width, height, and pixel format.
//! 2) Call
//!    [`get_zlib_decompression_requirement`](IHDR::get_zlib_decompression_requirement)
//!    to determine how much temporary space you'll need for the Zlib
//!    decompression and obtain an appropriate buffer. Because of how PNG works
//!    you *cannot* decompress directly to the final image buffer (other
//!    non-image data is mixed in).
//! 3) Call [`png_get_idat`](png_get_idat) to get an iterator over the
//!    compressed image data slices. PNG allows for more than one `IDAT` chunk
//!    within an image, and you should act like all `IDAT` chunks were a single
//!    long slice for the purposes of decompression. It's suggested to use the
//!    [`decompress_slice_iter_to_slice`](miniz_oxide::inflate::decompress_slice_iter_to_slice)
//!    function, but any Zlib decompressor will work. This gives you *filtered*
//!    data, not the final data you want.
//! 4) Depending on your intended final pixel format, allocate an appropriate
//!    buffer for the final image.
//! 5) Call [`unfilter_decompressed_data`](IHDR::unfilter_decompressed_data) on
//!    the decompressed data buffer to turn the decompressed but filtered data
//!    into the actual final pixel data. You provide this function with a
//!    closure `op(x, y, data)` that will be called once for each output pixel:
//!    * Bit depths 1, 2, and 4 will have the value in the low bits of a single
//!      byte slice.
//!    * Bit depth 8 will have one byte per channel.
//!    * Bit depth 16 will have two big-endian bytes per channel.
//!
//! ## Parsing Errors
//!
//! Quoting [section 13.2 of the PNG
//! spec](https://www.w3.org/TR/2003/REC-PNG-20031110/#13Decoders.Errors):
//!
//! > Errors that have little or no effect on the processing of the image may be
//! > ignored, while those that affect critical data shall be dealt with in a
//! > manner appropriate to the application.
//!
//! In our case, that means that we ignore as many spec violations as we
//! possibly can when parsing. Particularly, we ignore:
//!
//! * When the first 8 bytes of the data stream, marking it as PNG data, are
//!   incorrect. You can call [`is_png_header_correct`] yourself if you want to
//!   check the PNG header. The [PngRawChunkIter] will just skip the first 8
//!   bytes of input, regardless of if they're correct or not. If they're not
//!   correct, you probably don't have PNG bytes, and the chunks that the
//!   iterator produces will probably be nonsense, but won't break memory
//!   safety, or even panic, so basically it's kinda fine.
//! * All the chunk ordering rules. These exist to allow for potential PNG
//!   stream processing, but this library assumes that all PNG data is in memory
//!   at once anyway. This library processes chunks via Iterator, so it's fairly
//!   trivial to `filter` past chunks that occur in an unexpected order.
//! * Rules against duplicate chunks (you'll generally get the first one).
//! * Both of the checksum systems (CRC32 checks on individual chunks, and
//!   Adler32 checking on the Zlib compressed image data). These are basically
//!   there because PNG comes from an era (1996) when disks and networks were a
//!   lot less capable of preserving your data.

use crate::{
  ascii_array::AsciiArray,
  int_endian::{U16BE, U32BE},
  sRGBIntent,
};
use bitfrob::u8_replicate_bits;
use core::fmt::{Debug, Write};
use pixel_formats::{r8g8b8_Srgb, r8g8b8a8_Srgb};

mod actl;
mod bkgd;
mod chrm;
mod cicp;
mod crc32;
mod exif;
mod fctl;
mod fdat;
mod gama;
mod hist;
mod iccp;
mod idat;
mod iend;
mod ihdr;
mod itxt;
mod phys;
mod plte;
mod sbit;
mod splt;
mod srgb;
mod text;
mod time;
mod trns;
mod ztxt;

pub use self::{
  actl::*, bkgd::*, chrm::*, cicp::*, crc32::*, exif::*, fctl::*, fdat::*, gama::*, hist::*,
  iccp::*, idat::*, iend::*, ihdr::*, itxt::*, phys::*, plte::*, sbit::*, splt::*, srgb::*,
  text::*, time::*, trns::*, ztxt::*,
};
