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
//! Once the iterator is ready you need to get the header data. This comes in
//! the form of an [`IHDR`] chunk, and it should be the very first chunk you
//! find. Assuming that you're inside of a function that returns `Result<_,
//! PngError>` you'd use a few `?` operators here and there, something like
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
//!
//! Now you have the PNG's header information. This tells you:
//! * The dimensions of the image
//! * The pixel format of the image's pixels
//! * If the decompressed data of the image is interlaced or not.
//!
//! When storing the PNG, the raw pixel values are first "filtered" (to try and
//! make them more compression-friendly), and then compressed into a Zlib data
//! stream. To decode the PNG info you have to reverse the operations. First
//! decompressing, and then unfiltering.
//!
//! * **Unfiltering:** The decompressed data will be a series of lines for
//!   images with an extra byte on the front of each line which says what filter
//!   method was used for that line. The unfiltering can be performed in place,
//!   but leaves the filter markers between each line of useful pixel data. Most
//!   other code doesn't expect this layout, so you'll usually have to copy the
//!   lines into a different buffer.
//! * **De-interlacing:** If the image is interlaced then the data won't be
//!   stored as a single series of filtered bytes and lines. Instead, the
//!   overall image is stored as a series seven "reduced" images of varying
//!   resolutions. Again, most code doesn't expect this arrangement of the data,
//!   so you'll usually have to de-interlace the data to make it usable.
//! * **Flipping:** The PNG format assumes that the origin pixel is the top
//!   left, with scanlines going left to right, top to bottom. If your use of
//!   the data doesn't have this same assumption you'll need to flip the rows
//!   and/or columns of the pixels.
//! * **Pixel Format Changes:** The PNG's stored pixel format might not be the
//!   same as your desired target pixel format. Particularly, any pixel format
//!   that packs multiple pixels within a byte is unlikely to be usable by
//!   common code.
//!
//! The decompression is generally done as its own stage of work.
//!
//! All the other steps (unfiltering, de-interlacing, etc) can generally be
//! combined into just one additional pass over the decompressed data that
//! unfilters the data in place while also optionally passing out info to a
//! callback which places unfiltered pixels into a final buffer.

mod chunks;
pub use chunks::*;

// TODO: a data type for by-reference PNGs

// TODO: utilities for easily decoding PNG bytes into a by-ref PNG.

// TODO: a data type for owned PNGs

// TODO: utilities for easily decoding PNG bytes into owned PNG.

// TODO: utilities for converting any PNG pixel data into RGBA8 data.
