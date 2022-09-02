#![forbid(unsafe_code)]

//! Module for working with PNG data.
//!
//! * [Portable Network Graphics Specification (Second Edition)][png-spec]
//!
//! [png-spec]: https://www.w3.org/TR/2003/REC-PNG-20031110/
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

use core::fmt::{Debug, Write};

use bitfrob::u8_replicate_bits;

use crate::pixel_formats::{RGBA8888, RGB888};

// TODO: CRC support for raw chunks is needed later to write PNG data.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
struct PngRawChunkType([u8; 4]);
#[allow(nonstandard_style)]
impl PngRawChunkType {
  pub const IHDR: Self = Self(*b"IHDR");
  pub const PLTE: Self = Self(*b"PLTE");
  pub const IDAT: Self = Self(*b"IDAT");
  pub const IEND: Self = Self(*b"IEND");
  pub const tRNS: Self = Self(*b"tRNS");
}
impl Debug for PngRawChunkType {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_char(self.0[0] as char)?;
    f.write_char(self.0[1] as char)?;
    f.write_char(self.0[2] as char)?;
    f.write_char(self.0[3] as char)?;
    Ok(())
  }
}

/// An unparsed chunk from a PNG.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PngRawChunk<'b> {
  type_: PngRawChunkType,
  data: &'b [u8],
  declared_crc: u32,
}
impl Debug for PngRawChunk<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("PngRawChunk")
      .field("type_", &self.type_)
      .field("data", &(&self.data[..self.data.len().min(12)], self.data.len()))
      .field("declared_crc", &self.declared_crc)
      .finish()
  }
}

/// An iterator that produces successive raw chunks from PNG bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PngRawChunkIter<'b>(&'b [u8]);
impl<'b> PngRawChunkIter<'b> {
  /// Pass the full PNG bytes, it will remove the PNG header automatically.
  pub const fn new(bytes: &'b [u8]) -> Self {
    match bytes {
      [_, _, _, _, _, _, _, _, rest @ ..] => Self(rest),
      _ => Self(&[]),
    }
  }
}
impl<'b> Iterator for PngRawChunkIter<'b> {
  type Item = PngRawChunk<'b>;
  fn next(&mut self) -> Option<Self::Item> {
    let chunk_len: u32 = if self.0.len() >= 4 {
      let (len_bytes, rest) = self.0.split_at(4);
      self.0 = rest;
      u32::from_be_bytes(len_bytes.try_into().unwrap())
    } else {
      return None;
    };
    let type_: PngRawChunkType = if self.0.len() >= 4 {
      let (type_bytes, rest) = self.0.split_at(4);
      self.0 = rest;
      PngRawChunkType(type_bytes.try_into().unwrap())
    } else {
      return None;
    };
    let data: &'b [u8] = if self.0.len() >= chunk_len as usize {
      let (data, rest) = self.0.split_at(chunk_len as usize);
      self.0 = rest;
      data
    } else {
      return None;
    };
    let declared_crc: u32 = if self.0.len() >= 4 {
      let (decl_bytes, rest) = self.0.split_at(4);
      self.0 = rest;
      u32::from_be_bytes(decl_bytes.try_into().unwrap())
    } else {
      return None;
    };
    Some(PngRawChunk { type_, data, declared_crc })
  }
}

/// A parsed PNG chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
pub enum PngChunk<'b> {
  /// Image Header
  IHDR(IHDR),
  /// Palette
  PLTE(PLTE<'b>),
  /// Transparency
  tRNS(tRNS<'b>),
  /// Image Data
  IDAT(IDAT<'b>),
  /// Image End
  IEND,
}
impl<'b> TryFrom<PngRawChunk<'b>> for PngChunk<'b> {
  type Error = PngRawChunk<'b>;
  fn try_from(raw: PngRawChunk<'b>) -> Result<Self, Self::Error> {
    Ok(match raw.type_ {
      PngRawChunkType::IHDR => {
        // this can fail, so use `return` to avoid the outer Ok()
        return IHDR::try_from(raw.data).map(PngChunk::IHDR).map_err(|_| raw);
      }
      PngRawChunkType::PLTE => match bytemuck::try_cast_slice::<u8, RGB888>(raw.data) {
        Ok(entries) => PngChunk::PLTE(PLTE::from(entries)),
        Err(_) => return Err(raw),
      },
      PngRawChunkType::tRNS => PngChunk::tRNS(tRNS::from(raw.data)),
      PngRawChunkType::IDAT => PngChunk::IDAT(IDAT::from(raw.data)),
      PngRawChunkType::IEND => PngChunk::IEND,
      _ => return Err(raw),
    })
  }
}

/// The types of color that PNG supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PngColorType {
  /// Greyscale
  Y = 0,
  /// Red, Green, Blue
  RGB = 2,
  /// Index into a palette.
  ///
  /// The palette will have RGB8 data. There may optionally be a transparency
  /// chunk.
  Index = 3,
  /// Greyscale + Alpha
  YA = 4,
  /// Red, Green, Blue, Alpha
  RGBA = 6,
}
impl PngColorType {
  /// The number of channels in this type of color.
  pub const fn channel_count(self) -> usize {
    match self {
      Self::Y => 1,
      Self::RGB => 3,
      Self::Index => 1,
      Self::YA => 2,
      Self::RGBA => 4,
    }
  }
}
impl TryFrom<u8> for PngColorType {
  type Error = ();
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    Ok(match value {
      0 => PngColorType::Y,
      2 => PngColorType::RGB,
      3 => PngColorType::Index,
      4 => PngColorType::YA,
      6 => PngColorType::RGBA,
      _ => return Err(()),
    })
  }
}

/// Image Header
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IHDR {
  /// width in pixels
  pub width: u32,
  /// height in pixels
  pub height: u32,
  /// bits per channel
  pub bit_depth: u8,
  /// pixel color type
  pub color_type: PngColorType,
  /// if the image data is stored interlaced.
  ///
  /// please don't make new interlaced images, they're terrible.
  pub is_interlaced: bool,
}
impl IHDR {
  /// You can call this if you must, but it complicates the apparent API to have
  /// it visible because most people don't ever need this.
  #[doc(hidden)]
  pub const fn bytes_per_filterline(&self, width: u32) -> usize {
    // each line is a filter byte (1) + pixel data. When pixels are less than 8
    // bits per channel it's possible to end up with partial bytes on the end,
    // so we must round up.
    1 + ((self.bits_per_pixel() * (width as usize)) + 7) / 8
  }

  /// Gets the buffer size required to perform Zlib decompression.
  pub fn get_zlib_decompression_requirement(&self) -> usize {
    /// Get the temp bytes for a given image.
    ///
    /// * Interlaced images will have to call this function for all 7 reduced
    ///   images and then add up the values.
    /// * Non-interlaced images call this function just once for their full
    ///   dimensions.
    #[inline]
    #[must_use]
    fn temp_bytes_for_image(
      width: u32, height: u32, color_type: PngColorType, bit_depth: u8,
    ) -> usize {
      if width == 0 {
        return 0;
      }
      let bits_per_pixel: usize = color_type.channel_count().saturating_mul(bit_depth as usize);
      let bits_per_line: usize = bits_per_pixel.saturating_mul(width as usize);
      let bytes_per_scanline: usize = (bits_per_line / 8) + (bits_per_line % 8 != 0) as usize;
      let bytes_per_filterline: usize = bytes_per_scanline.saturating_add(1);
      bytes_per_filterline.saturating_mul(height as usize)
    }
    if self.is_interlaced {
      let mut total = 0_usize;
      for (width, height) in reduced_image_dimensions(self.width, self.height) {
        total = total.saturating_add(temp_bytes_for_image(
          width,
          height,
          self.color_type,
          self.bit_depth,
        ));
      }
      total
    } else {
      temp_bytes_for_image(self.width, self.height, self.color_type, self.bit_depth)
    }
  }

  /// You can call this if you must, but it complicates the apparent API to have
  /// it visible because most people don't ever need this.
  #[doc(hidden)]
  pub const fn bits_per_pixel(&self) -> usize {
    (self.bit_depth as usize) * self.color_type.channel_count()
  }
}
impl TryFrom<PngChunk<'_>> for IHDR {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'_>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::IHDR(ihdr) => Ok(ihdr),
      _ => Err(()),
    }
  }
}
impl TryFrom<&[u8]> for IHDR {
  type Error = ();
  fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
    match value {
      [w0, w1, w2, w3, h0, h1, h2, h3, bit_depth, color_type, _compression_method, _filter_method, interlace_method] => {
        Ok(Self {
          width: u32::from_be_bytes([*w0, *w1, *w2, *w3]),
          height: u32::from_be_bytes([*h0, *h1, *h2, *h3]),
          bit_depth: match *color_type {
            0 if [1, 2, 4, 8, 16].contains(bit_depth) => *bit_depth,
            2 if [8, 16].contains(bit_depth) => *bit_depth,
            3 if [1, 2, 4, 8].contains(bit_depth) => *bit_depth,
            4 if [8, 16].contains(bit_depth) => *bit_depth,
            6 if [8, 16].contains(bit_depth) => *bit_depth,
            _ => return Err(()),
          },
          color_type: PngColorType::try_from(*color_type)?,
          is_interlaced: match interlace_method {
            0 => false,
            1 => true,
            _ => return Err(()),
          },
        })
      }
      _ => Err(()),
    }
  }
}

/// Transparency data
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
pub struct tRNS<'b>(&'b [u8]);
impl<'b> From<&'b [u8]> for tRNS<'b> {
  #[inline]
  #[must_use]
  fn from(data: &'b [u8]) -> Self {
    Self(data)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for tRNS<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::tRNS(trns) => Ok(trns),
      _ => Err(()),
    }
  }
}
impl Debug for tRNS<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("tRNS").field(&&self.0[..]).field(&self.0.len()).finish()
  }
}
impl<'b> tRNS<'b> {
  /// Gets the grayscale value that is transparent.
  ///
  /// Fails when the chunk has the wrong length for grayscale.
  #[inline]
  pub const fn try_to_grayscale(&self) -> Option<u16> {
    match self.0 {
      [y0, y1] => Some(u16::from_be_bytes([*y0, *y1])),
      _ => None,
    }
  }
  /// Gets the RGB value that is transparent.
  ///
  /// Fails when the chunk has the wrong length for rgb.
  #[inline]
  pub const fn try_to_rgb(&self) -> Option<[u16; 3]> {
    match self.0 {
      [r0, r1, g0, g1, b0, b1] => Some([
        u16::from_be_bytes([*r0, *r1]),
        u16::from_be_bytes([*g0, *g1]),
        u16::from_be_bytes([*b0, *b1]),
      ]),
      _ => None,
    }
  }
  /// Gets the alpha values for each palette index.
  pub const fn to_alphas(&self) -> &'b [u8] {
    self.0
  }
}

/// Palette data
///
/// Palette entries are always RGB.
///
/// If you want to have a paletted image with transparency then the transparency
/// info goes in a separate transparency chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PLTE<'b>(&'b [RGB888]);
impl<'b> From<&'b [RGB888]> for PLTE<'b> {
  #[inline]
  #[must_use]
  fn from(entries: &'b [RGB888]) -> Self {
    Self(entries)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for PLTE<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::PLTE(plte) => Ok(plte),
      _ => Err(()),
    }
  }
}
impl Debug for PLTE<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    // currently prints no more than 4 palette entries
    f.debug_tuple("PLTE").field(&&self.0[..self.0.len().min(4)]).field(&self.0.len()).finish()
  }
}
impl<'b> PLTE<'b> {
  /// Gets the entries as a slice.
  pub fn entries(&self) -> &'b [RGB888] {
    self.0
  }
}

/// Image Data.
///
/// * Image data is stored with Zlib compression applied.
/// * Images can have more than one IDAT chunk. They should all be stored in a
///   row. Multiple chunks are treated as a single Zlib datastream.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IDAT<'b>(&'b [u8]);
impl<'b> From<&'b [u8]> for IDAT<'b> {
  #[inline]
  #[must_use]
  fn from(data: &'b [u8]) -> Self {
    Self(data)
  }
}
impl<'b> TryFrom<PngChunk<'b>> for IDAT<'b> {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'b>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::IDAT(idat) => Ok(idat),
      _ => Err(()),
    }
  }
}
impl Debug for IDAT<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("IDAT").field(&&self.0[..self.0.len().min(12)]).field(&self.0.len()).finish()
  }
}
impl<'b> IDAT<'b> {
  fn as_bytes(&self) -> &'b [u8] {
    self.0
  }
}

/// Checks if the PNG's initial 8 bytes are correct.
///
/// * If this is the case, the rest of the bytes are very likely PNG data.
/// * If this is *not* the case, the rest of the bytes are very likely *not* PNG
///   data.
pub const fn is_png_header_correct(bytes: &[u8]) -> bool {
  matches!(bytes, [137, 80, 78, 71, 13, 10, 26, 10, ..])
}

/// Gets the [IHDR] out of the PNG bytes.
pub fn png_get_header(bytes: &[u8]) -> Option<IHDR> {
  PngRawChunkIter::new(bytes)
    .filter_map(|raw_chunk| {
      let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
      IHDR::try_from(png_chunk).ok()
    })
    .next()
}

/// Gets the transparency chunk for the PNG bytes, if any.
pub fn png_get_transparency(bytes: &[u8]) -> Option<tRNS<'_>> {
  PngRawChunkIter::new(bytes).filter_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let trns = tRNS::try_from(png_chunk).ok()?;
    Some(trns)
  }).next()
}

/// Gets the palette out of the PNG bytes.
///
/// Each `[u8;3]` in the palette is an `[r8, g8, b8]` color entry.
pub fn png_get_palette(bytes: &[u8]) -> Option<&[RGB888]> {
  PngRawChunkIter::new(bytes)
    .filter_map(|raw_chunk| {
      let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
      let plte = PLTE::try_from(png_chunk).ok()?;
      Some(plte.entries())
    })
    .next()
}

/// Gets an iterator over all the [IDAT] slices in the PNG bytes.
pub fn png_get_idat(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
  PngRawChunkIter::new(bytes).filter_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let idat = IDAT::try_from(png_chunk).ok()?;
    Some(idat.as_bytes())
  })
}

/// Given the dimensions of the full PNG image, computes the size of each
/// reduced image.
///
/// The PNG interlacing scheme converts a full image to 7 reduced images, each
/// with potentially separate dimensions. Knowing the size of each reduced image
/// is important for the unfiltering process.
///
/// The output uses index 0 as the base image size, and indexes 1 through 7 for
/// the size of reduced images 1 through 7.
///
/// PS: Interlacing is terrible, don't interlace your images.
#[inline]
#[must_use]
const fn reduced_image_dimensions(full_width: u32, full_height: u32) -> [(u32, u32); 8] {
  // ```
  // 1 6 4 6 2 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // 3 6 4 6 3 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // ```
  let full_patterns_wide = full_width / 8;
  let full_patterns_high = full_height / 8;
  //
  let partial_pattern_width = full_width % 8;
  let partial_pattern_height = full_height % 8;
  //
  let zero = (full_width, full_height);
  //
  let first = (
    full_patterns_wide + (partial_pattern_width + 7) / 8,
    full_patterns_high + (partial_pattern_height + 7) / 8,
  );
  let second = (
    full_patterns_wide + (partial_pattern_width + 3) / 8,
    full_patterns_high + (partial_pattern_height + 7) / 8,
  );
  let third = (
    full_patterns_wide * 2 + ((partial_pattern_width + 3) / 4),
    full_patterns_high + ((partial_pattern_height + 3) / 8),
  );
  let fourth = (
    full_patterns_wide * 2 + (partial_pattern_width + 1) / 4,
    full_patterns_high * 2 + (partial_pattern_height + 3) / 4,
  );
  let fifth = (
    full_patterns_wide * 4 + ((partial_pattern_width + 1) / 2),
    full_patterns_high * 2 + (partial_pattern_height + 1) / 4,
  );
  let sixth = (
    full_patterns_wide * 4 + partial_pattern_width / 2,
    full_patterns_high * 4 + ((partial_pattern_height + 1) / 2),
  );
  let seventh = (
    full_patterns_wide * 8 + partial_pattern_width,
    full_patterns_high * 4 + (partial_pattern_height / 2),
  );
  //
  [zero, first, second, third, fourth, fifth, sixth, seventh]
}

#[test]
fn test_reduced_image_dimensions() {
  assert_eq!(reduced_image_dimensions(0, 0), [(0, 0); 8]);
  // one
  for (w, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(w, 0)[1].0, ex, "failed w:{}", w);
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[1].1, ex, "failed h:{}", h);
  }
  // two
  for (w, ex) in (1..=8).zip([0, 0, 0, 0, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(w, 0)[2].0, ex, "failed w:{}", w);
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[2].1, ex, "failed h:{}", h);
  }
  // three
  for (w, ex) in (1..=8).zip([1, 1, 1, 1, 2, 2, 2, 2]) {
    assert_eq!(reduced_image_dimensions(w, 0)[3].0, ex, "failed w: {}", w);
  }
  for (h, ex) in (1..=8).zip([0, 0, 0, 0, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[3].1, ex, "failed h: {}", h);
  }
  // four
  for (w, ex) in (1..=8).zip([0, 0, 1, 1, 1, 1, 2, 2]) {
    assert_eq!(reduced_image_dimensions(w, 0)[4].0, ex, "failed w: {}", w);
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 2, 2, 2, 2]) {
    assert_eq!(reduced_image_dimensions(0, h)[4].1, ex, "failed h: {}", h);
  }
  // five
  for (w, ex) in (1..=8).zip([1, 1, 2, 2, 3, 3, 4, 4]) {
    assert_eq!(reduced_image_dimensions(w, 0)[5].0, ex, "failed w: {}", w);
  }
  for (h, ex) in (1..=8).zip([0, 0, 1, 1, 1, 1, 2, 2]) {
    assert_eq!(reduced_image_dimensions(0, h)[5].1, ex, "failed h: {}", h);
  }
  // six
  for (w, ex) in (1..=8).zip([0, 1, 1, 2, 2, 3, 3, 4]) {
    assert_eq!(reduced_image_dimensions(w, 0)[6].0, ex, "failed w: {}", w);
  }
  for (h, ex) in (1..=8).zip([1, 1, 2, 2, 3, 3, 4, 4]) {
    assert_eq!(reduced_image_dimensions(0, h)[6].1, ex, "failed h: {}", h);
  }
  // seven
  for (w, ex) in (1..=8).zip([1, 2, 3, 4, 5, 6, 7, 8]) {
    assert_eq!(reduced_image_dimensions(w, 0)[7].0, ex, "failed w: {}", w);
  }
  for (h, ex) in (1..=8).zip([0, 1, 1, 2, 2, 3, 3, 4]) {
    assert_eq!(reduced_image_dimensions(0, h)[7].1, ex, "failed h: {}", h);
  }
  //
  assert_eq!(
    reduced_image_dimensions(8, 8),
    [
      (8, 8), // zeroth
      (1, 1), // one
      (1, 1), // two
      (2, 1), // three
      (2, 2), // four
      (4, 2), // five
      (4, 4), // six
      (8, 4), // seven
    ]
  );
}

/// Converts a reduced image location into the full image location.
///
/// For consistency between this function and the [reduced_image_dimensions]
/// function, when giving an `image_level` of 0 the output will be the same as
/// the input.
///
/// ## Panics
/// * If the image level given exceeds 7.
#[inline]
#[must_use]
const fn interlaced_pos_to_full_pos(
  image_level: usize, reduced_x: u32, reduced_y: u32,
) -> (u32, u32) {
  // ```
  // 1 6 4 6 2 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // 3 6 4 6 3 6 4 6
  // 7 7 7 7 7 7 7 7
  // 5 6 5 6 5 6 5 6
  // 7 7 7 7 7 7 7 7
  // ```
  #[allow(clippy::identity_op)]
  match image_level {
    0 /* full image */ => (reduced_x, reduced_y),
    1 => (reduced_x * 8 + 0, reduced_y * 8 + 0),
    2 => (reduced_x * 8 + 4, reduced_y * 8 + 0),
    3 => (reduced_x * 4 + 0, reduced_y * 8 + 4),
    4 => (reduced_x * 4 + 2, reduced_y * 4 + 0),
    5 => (reduced_x * 2 + 0, reduced_y * 4 + 2),
    6 => (reduced_x * 2 + 1, reduced_y * 2 + 0),
    7 => (reduced_x * 1 + 0, reduced_y * 2 + 1),
    _ => panic!("reduced image level must be 1 through 7")
  }
}

impl IHDR {
  fn send_out_pixel<F: FnMut(u32, u32, &[u8])>(
    &self, image_level: usize, reduced_x: u32, reduced_y: u32, data: &[u8], op: &mut F,
  ) {
    let full_width = self.width;
    match self.bit_depth {
      1 => {
        let full_data: u8 = data[0];
        let mut mask = 0b1000_0000;
        let mut down_shift = 7;
        for plus_x in 0..8 {
          let (image_x, image_y) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 8 + plus_x, reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
          mask >>= 1;
          down_shift -= 1;
        }
      }
      2 => {
        let full_data: u8 = data[0];
        let mut mask = 0b1100_0000;
        let mut down_shift = 6;
        for plus_x in 0..4 {
          let (image_x, image_y) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 4 + plus_x, reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
          mask >>= 2;
          down_shift -= 2;
        }
      }
      4 => {
        let full_data: u8 = data[0];
        let mut mask = 0b1111_0000;
        let mut down_shift = 4;
        for plus_x in 0..2 {
          let (image_x, image_y) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 2 + plus_x, reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x as u32, image_y as u32, &[(full_data & mask) >> down_shift]);
          mask >>= 4;
          down_shift -= 4;
        }
      }
      8 | 16 => {
        let (image_x, image_y) = interlaced_pos_to_full_pos(image_level, reduced_x, reduced_y);
        op(image_x as u32, image_y as u32, data);
      }
      _ => unreachable!(),
    }
  }

  /// Unfilters data from the zlib decompression buffer into the final
  /// destination.
  ///
  /// See the [`png` module docs](crate::png) for guidance.
  #[allow(clippy::result_unit_err)]
  pub fn unfilter_decompressed_data<F>(
    &self, mut decompressed: &mut [u8], mut op: F,
  ) -> Result<(), ()>
  where
    F: FnMut(u32, u32, &[u8]),
  {
    #[inline]
    #[must_use]
    const fn paeth_predict(a: u8, b: u8, c: u8) -> u8 {
      let a_ = a as i32;
      let b_ = b as i32;
      let c_ = c as i32;
      let p: i32 = a_ + b_ - c_;
      let pa = (p - a_).abs();
      let pb = (p - b_).abs();
      let pc = (p - c_).abs();
      // Note(Lokathor): The PNG spec is extremely specific that you shall not,
      // under any circumstances, alter the order of evaluation of this
      // expression's tests.
      if pa <= pb && pa <= pc {
        a
      } else if pb <= pc {
        b
      } else {
        c
      }
    }

    if self.width == 0 || self.height == 0 {
      return Err(());
    }

    // filtering is per byte within a pixel when pixels are more than 1 byte
    // each, and per byte when pixels are 1 byte or less.
    let filter_chunk_size = match self.color_type {
      PngColorType::Y => match self.bit_depth {
        16 => 2,
        8|4|2|1=>1,
        _=> return Err(()),
      }
      PngColorType::RGB => match self.bit_depth {
        8 => 3,
        16 => 6,
        _ => return Err(()),
      },
      PngColorType::Index => match self.bit_depth{
        8|4|2|1=> 1,
        _=> return Err(()),
      },
      PngColorType::YA => match self.bit_depth {
        8 => 2,
        16 => 4,
        _ => return Err(()),
      },
      PngColorType::RGBA => match self.bit_depth {
        8 => 4,
        16 => 8,
        _ => return Err(()),
      },
    };

    // The image is either interlaced or not:
    // * when interlaced, we will work through "reduced images" 1 through 7.
    // * then not interlaced, we will use just the main image.
    let mut image_it = reduced_image_dimensions(self.width, self.height)
      .into_iter()
      .enumerate()
      .map(|(i, (w, h))| (i, w, h))
      .take(if self.is_interlaced { 8 } else { 1 });
    if self.is_interlaced {
      image_it.next();
    }

    // From now on we're "always" working with reduced images because we've
    // re-stated the non-interlaced scenario as being just a form of interlaced
    // data, which means we can stop thinking about the difference between if
    // we're interlaced or not. yay!
    for (image_level, reduced_width, reduced_height) in image_it {
      if reduced_width == 0 || reduced_height == 0 {
        // while the full image's width and height must not be 0, the width or
        // height of any particular reduced image might still be 0. In that case, we
        // just continue on.
        continue;
      }

      let bytes_per_filterline = self.bytes_per_filterline(reduced_width);
      let bytes_used_this_image = bytes_per_filterline.saturating_mul(reduced_height as _);

      let mut row_iter = if decompressed.len() < bytes_used_this_image {
        return Err(());
      } else {
        let (these_bytes, more_bytes) = decompressed.split_at_mut(bytes_used_this_image);
        decompressed = more_bytes;
        these_bytes
          .chunks_exact_mut(bytes_per_filterline)
          .map(|chunk| {
            let (f, pixels) = chunk.split_at_mut(1);
            (&mut f[0], pixels)
          })
          .enumerate()
          .take(reduced_height as usize)
          .map(|(r_y, (f, pixels))| (r_y as u32, f, pixels))
      };

      // The first line of each image has special handling because filters can
      // refer to the previous line, but for the first line the "previous line" is
      // an implied zero.
      let mut b_pixels = if let Some((reduced_y, f, pixels)) = row_iter.next() {
        let mut p_it =
          pixels.chunks_exact_mut(filter_chunk_size).enumerate().map(|(r_x, d)| (r_x as u32, d));
        match f {
          1 => {
            // Sub
            let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel = pixel;
            for (reduced_x, pixel) in p_it {
              a_pixel
                .iter()
                .copied()
                .zip(pixel.iter_mut())
                .for_each(|(a, p)| *p = p.wrapping_add(a));
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          3 => {
            // Average
            let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel = pixel;
            for (reduced_x, pixel) in p_it {
              // the `b` is always 0, so we elide it from the computation
              a_pixel
                .iter()
                .copied()
                .zip(pixel.iter_mut())
                .for_each(|(a, p)| *p = p.wrapping_add(a / 2));
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          4 => {
            // Paeth
            let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel = pixel;
            for (reduced_x, pixel) in p_it {
              // the `b` and `c` are both always 0
              a_pixel
                .iter()
                .copied()
                .zip(pixel.iter_mut())
                .for_each(|(a, p)| *p = p.wrapping_add(paeth_predict(a, 0, 0)));
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          _ => {
            for (reduced_x, pixel) in p_it {
              // None and Up
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            }
          }
        }
        *f = 0;
        pixels
      } else {
        unreachable!("we already know that this image is at least 1 row");
      };

      // Now that we have a previous line worth of data, all the filters will work
      // normally for the rest of the image.
      for (reduced_y, f, pixels) in row_iter {
        let mut p_it =
          pixels.chunks_exact_mut(filter_chunk_size).enumerate().map(|(r_x, d)| (r_x as u32, d));
        let b_it = b_pixels.chunks_exact(filter_chunk_size);
        match f {
          1 => {
            // Sub filter
            let (reduced_x, pixel): (u32, &mut [u8]) = p_it.next().unwrap();
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel = pixel;
            for (reduced_x, pixel) in p_it {
              a_pixel
                .iter()
                .copied()
                .zip(pixel.iter_mut())
                .for_each(|(a, p)| *p = p.wrapping_add(a));
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          2 => {
            // Up filter
            for ((reduced_x, pixel), b_pixel) in p_it.zip(b_it) {
              b_pixel
                .iter()
                .copied()
                .zip(pixel.iter_mut())
                .for_each(|(b, p)| *p = p.wrapping_add(b));
              //
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            }
          }
          3 => {
            // Average filter
            let mut pb_it = p_it.zip(b_it).map(|((r_x, p), b)| (r_x, p, b));
            let (reduced_x, pixel, b_pixel) = pb_it.next().unwrap();
            pixel
              .iter_mut()
              .zip(b_pixel.iter().copied())
              .for_each(|(p, b)| *p = p.wrapping_add(b / 2));
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel: &[u8] = pixel;
            for (reduced_x, pixel, b_pixel) in pb_it {
              a_pixel.iter().copied().zip(b_pixel.iter().copied()).zip(pixel.iter_mut()).for_each(
                |((a, b), p)| {
                  *p = p.wrapping_add(((a as u32 + b as u32) / 2) as u8);
                },
              );
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
            }
          }
          4 => {
            // Paeth filter
            let mut pb_it = p_it.zip(b_it).map(|((r_x, p), b)| (r_x, p, b));
            let (reduced_x, pixel, b_pixel) = pb_it.next().unwrap();
            pixel.iter_mut().zip(b_pixel.iter().copied()).for_each(|(p, b)| {
              *p = p.wrapping_add(paeth_predict(0, b, 0));
            });
            self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            let mut a_pixel = pixel;
            let mut c_pixel = b_pixel;
            for (reduced_x, pixel, b_pixel) in pb_it {
              a_pixel
                .iter()
                .copied()
                .zip(b_pixel.iter().copied())
                .zip(c_pixel.iter().copied())
                .zip(pixel.iter_mut())
                .for_each(|(((a, b), c), p)| {
                  *p = p.wrapping_add(paeth_predict(a, b, c));
                });
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
              //
              a_pixel = pixel;
              c_pixel = b_pixel;
            }
          }
          _ => {
            for (reduced_x, pixel) in p_it {
              // No Filter, or unknown filter, have no alterations.
              self.send_out_pixel(image_level, reduced_x, reduced_y, pixel, &mut op);
            }
          }
        }
        b_pixels = pixels;
      }
    }

    //
    Ok(())
  }
}

#[cfg(all(feature = "alloc", feature = "miniz_oxide"))]
impl<P> crate::image::Image<P>
where
  P: From<RGBA8888> + Clone,
{
  /// Attempts to make an image from PNG bytes.
  ///
  /// ## Failure
  /// Errors include, but are not limited to:
  /// * Allocation failure.
  /// * No [IHDR] found in the bytes.
  ///
  /// There's currently no error reporting, you just get `None`.
  #[cfg_attr(docs_rs, doc(cfg(all(feature = "png", feature = "miniz_oxide"))))]
  pub fn try_from_png_bytes(bytes: &[u8]) -> Option<Self> {
    use alloc::vec::Vec;
    //
    let ihdr = match png_get_header(bytes) {
      Some(ihdr) => ihdr,
      None => {
        // No Image Header prevents all further processing.
        return None;
      }
    };
    let zlib_len = ihdr.get_zlib_decompression_requirement();
    let mut zlib_buffer: Vec<u8> = Vec::new();
    zlib_buffer.try_reserve(zlib_len).ok()?;
    // ferris plz make this into a memset
    zlib_buffer.resize(zlib_len, 0);
    match miniz_oxide::inflate::decompress_slice_iter_to_slice(
      &mut zlib_buffer,
      png_get_idat(bytes),
      true,
      true,
    ) {
      Ok(decompression_count) => {
        if decompression_count < zlib_buffer.len() {
          // potential a cause for concern, but i guess keep going. We already
          // put enough zeros into the zlib buffer so we'll be able to unfilter
          // either way.
        }
      }
      Err(_e) => {
        // should we cancel with an error? The most likely errors are that
        // there's not actually Zlib compressed data (so we'd have an image of
        // zeros) or there's too much Zlib compressed data (in which case we
        // can maybe proceed with partial results).
      }
    }
    let pixel_count = (ihdr.width * ihdr.height) as usize;
    let mut pixels: Vec<P> = Vec::new();
    pixels.try_reserve(pixel_count).ok()?;
    // ferris plz make this into a memset
    pixels.resize(pixel_count, RGBA8888::default().into());
    let mut image = Self { width: ihdr.width, height: ihdr.height, pixels };
    let plte: &[RGB888] = if ihdr.color_type == PngColorType::Index {
      png_get_palette(bytes).unwrap_or(&[])
    } else {
      &[]
    };
    let (trns_y, trns_rgb, trns_alphas): (Option<u16>, Option<[u16;3]>, Option<&[u8]>) = {
      if let Some(trns) = png_get_transparency(bytes) {
        (
          trns.try_to_grayscale(),
          trns.try_to_rgb(),
          Some(trns.to_alphas()),
        )
      } else {
        (None, None, None)
      }
    };
    let unfilter_op = |x: u32, y: u32, data: &[u8]| {
      if let Some(p) = image.get_mut(x, y) {
        match ihdr.color_type {
          PngColorType::RGB => {
            let [r, g, b] = if ihdr.bit_depth == 16 {
              [data[0], data[2], data[4]]
            } else {
              [data[0], data[1], data[2]]
            };
            let full = if ihdr.bit_depth == 16{ Some([
              u16::from_be_bytes([data[0], data[1]]),
              u16::from_be_bytes([data[2], data[3]]),
              u16::from_be_bytes([data[4], data[5]]),
            ])} else {
              Some([data[0] as u16, data[1] as u16, data[2] as u16])
            };
            let a = if trns_rgb == full {
              0
            } else {
              255
            };
            *p = RGBA8888 { r, g, b, a }.into();
          }
          PngColorType::RGBA => {
            let [r, g, b, a] = if ihdr.bit_depth == 16 {
              [data[0], data[2], data[4], data[6]]
            } else {
              [data[0], data[1], data[2], data[3]]
            };
            *p = RGBA8888 { r, g, b, a }.into();
          }
          PngColorType::YA => {
            let [y, a] = if ihdr.bit_depth == 16 { [data[0], data[2]] } else { [data[0], data[1]] };
            *p = RGBA8888 { r: y, g: y, b: y, a }.into();
          }
          PngColorType::Y => {
            let y = if ihdr.bit_depth == 16 {
              data[0]
            } else {
              u8_replicate_bits(ihdr.bit_depth as u32, data[0])
            };
            let full = if ihdr.bit_depth == 16 {Some(
              u16::from_be_bytes([data[0], data[1]]),
            )} else { Some(data[0] as u16) };
            let a = if trns_y == full {
              0
            } else {
              255
            };
            *p = RGBA8888 { r: y, g: y, b: y, a }.into();
          }
          PngColorType::Index => {
            let RGB888{r,g,b} = *plte.get(data[0] as usize).unwrap_or(&RGB888::default());
            let a = if let Some(alphas) = trns_alphas {
              *alphas.get(data[0] as usize).unwrap_or(&255)
            } else {
              255
            };
            *p = RGBA8888{r,g,b,a}.into()
          }
        }
      } else {
        // attempted out of bounds pixel write, but i guess it doesn't matter?
      }
    };
    if let Err(_e) = ihdr.unfilter_decompressed_data(&mut zlib_buffer, unfilter_op) {
      // err during unfiltering, do we care?
    }
    Some(image)
  }
}
