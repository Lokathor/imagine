//! Holds all the tools for decoding PNG data.
//!
//! ## Automated PNG Decoding
//! If you don't need full control over the decoding process there's functions
//! provided that will take a PNG data stream and just give you the pixels in a
//! `Vec<RGBA8>` (along with other info like width, height, etc).
//!
//! These functions do the allocation for you, and so they require that the
//! `alloc` feature be enabled.
//!
//! ## Decoding a PNG Yourself
//! This crate allows you to directly control the entire PNG decoding process
//! yourself if you wish. The advantage of doing this is that you can avoid any
//! excess allocation.
//!
//! The general format of a PNG is that the information is stored in "chunks".
//! You iterate the chunks and each one gives you some info that you might
//! decide to use. There's four "critical" chunk types:
//! * **Header** - This has all the important information about the image's
//!   dimensions, pixel format, and if the image is interlaced or not. Using
//!   this information you'll be able to know how much temporary space is
//!   required for decompression, and how much final space is required after
//!   unfiltering.
//! * **Palette** - If an image uses indexed color it will have a palette of
//!   what index values map to what `RGB8` values.
//! * **Image Data** - One or more chunks of compressed data. All of the
//!   compressed data forms a single zlib data stream. All of the image data
//!   chunks should appear one after the other.
//! * **End** - The last chunk, lets you know you had the full PNG and your data
//!   wasn't truncated accidentally.
//!
//! After the header and before the image data there are also zero or more
//! "ancillary" chunks which might give you additional information about the
//! image. If you just want to display the image, the ancillary chunk that's
//! most likely to be important to you is if there's a transparency chunk.
//!
//! ### Step By Step
//!
//! First you'll want an iterator over the PNG chunks. In this example, we use a
//! raw chunk iterator, parse each raw chunk into a more structured chunk value,
//! and then filter any errors for only the most critical errors using the
//! [`critical_errors_only`] helper filter.
//!
//! ```no_run
//! use imagine::png::*;
//! let png: &[u8] = unimplemented!("data from somewhere");
//! let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);
//! ```
//!
//! Once the iterator is ready you need to get the header data.
//! This comes in the form of an [`IHDR`] chunk, and it should be the very first
//! chunk you find. Assuming that you're inside of a function that returns
//! `Result<_, PngError>` you'd use a few `?` operators and have something like
//! this.
//!
//! ```no_run
//! # use imagine::png::*;
//! # fn or_png_error(png: &[u8]) -> Result<(), PngError> {
//! #  let mut it = RawPngChunkIter::new(&[]).map(PngChunk::try_from);
//! let ihdr: IHDR =
//!   it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;
//! # Ok(())
//! # }
//! ```

mod chunks;
pub use chunks::*;

// TODO: a data type for by-reference PNGs

// TODO: utilities for easily decoding PNG bytes into a by-ref PNG.

// TODO: a data type for owned PNGs

// TODO: utilities for easily decoding PNG bytes into owned PNG.

// TODO: utilities for converting any PNG pixel data into RGBA8 data.
