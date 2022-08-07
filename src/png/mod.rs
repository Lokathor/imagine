//! Module for working with PNG data.
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
//! This library does *not* attempt to support "stream" decoding of PNG data in
//! a way that keeps a minimal amount of data live during the decoding. It might
//! be possible to create such a thing using the types provided in this module,
//! but that's not an intended use case.
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
//!   incorrect. You can call [`is_png_header_correct`] yourself if you want.
//! * All the chunk ordering rules. All chunk processing is done via Iterator,
//!   so it's trivial to filter past chunks that occur in an unexpected order.
//! * Rules against duplicate chunks (you'll generally get the first one).
//! * Both of the checksum systems (CRC32 checks on individual chunks, and
//!   Adler32 checking on the Zlib compressed image data).

use core::fmt::{Debug, Write};

// TODO: CRC support for raw chunks is needed

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PngRawChunkType([u8; 4]);
impl PngRawChunkType {
  pub const IHDR: Self = Self(*b"IHDR");
  pub const PLTE: Self = Self(*b"PLTE");
  pub const IDAT: Self = Self(*b"IDAT");
  pub const IEND: Self = Self(*b"IEND");
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PngRawChunkIter<'b>(&'b [u8]);
impl<'b> PngRawChunkIter<'b> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PngChunk<'b> {
  IHDR(IHDR),
  PLTE(PLTE<'b>),
  IDAT(IDAT<'b>),
  IEND,
}
impl<'b> TryFrom<PngRawChunk<'b>> for PngChunk<'b> {
  type Error = PngRawChunk<'b>;
  fn try_from(raw: PngRawChunk<'b>) -> Result<Self, Self::Error> {
    Ok(match raw.type_ {
      PngRawChunkType::IHDR => {
        return IHDR::try_from(raw.data).map(PngChunk::IHDR).map_err(|_| raw);
      }
      PngRawChunkType::PLTE => match bytemuck::try_cast_slice::<u8, [u8; 3]>(raw.data) {
        Ok(entries) => PngChunk::PLTE(PLTE::from(entries)),
        Err(_) => return Err(raw),
      },
      PngRawChunkType::IDAT => PngChunk::IDAT(IDAT::from(raw.data)),
      PngRawChunkType::IEND => PngChunk::IEND,
      _ => return Err(raw),
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PngColorType {
  Y = 0,
  RGB = 2,
  Index = 3,
  YA = 4,
  RGBA = 6,
}
impl PngColorType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IHDR {
  width: u32,
  height: u32,
  bit_depth: u8,
  color_type: PngColorType,
  is_interlaced: bool,
}
impl IHDR {
  /// Gets the buffer size required to perform Zlib decompression.
  pub const fn get_zlib_decompression_requirement(&self) -> usize {
    // each line is a filter byte (1) + pixel data. When pixels are less than 8
    // bits per channel it's possible to end up with partial bytes on the end,
    // so we must round up.
    let bytes_per_line = 1 + ((self.bits_per_pixel() * (self.width as usize)) + 7) / 8;
    bytes_per_line * (self.height as usize)
  }

  /// bits per pixel = bit depth per channel * channels per pixel
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PLTE<'b>(&'b [[u8; 3]]);
impl<'b> From<&'b [[u8; 3]]> for PLTE<'b> {
  #[inline]
  #[must_use]
  fn from(entries: &'b [[u8; 3]]) -> Self {
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
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("PLTE").field(&&self.0[..self.0.len().min(4)]).field(&self.0.len()).finish()
  }
}
impl<'b> PLTE<'b> {
  pub fn entries(&self) -> &'b [[u8; 3]] {
    self.0
  }
}

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
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
  match bytes {
    [137, 80, 78, 71, 13, 10, 26, 10, ..] => true,
    _ => false,
  }
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

/// Gets the palette out of the PNG bytes.
///
/// Each `[u8;3]` in the palette is an `[r8, g8, b8]` color entry.
pub fn png_get_palette(bytes: &[u8]) -> Option<&[[u8; 3]]> {
  PngRawChunkIter::new(bytes)
    .filter_map(|raw_chunk| {
      let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
      let plte = PLTE::try_from(png_chunk).ok()?;
      Some(plte.entries())
    })
    .next()
}

/// Gets an iterator over all the [IDAT] slices in the PNG bytes.
///
/// The intended use is to:
///
/// 1) Determine how much buffer space you need to decompress the `IDAT` by
///    grabbing the [IHDR] for this image and then calling
///    [`get_zlib_decompression_requirement`](IHDR::get_zlib_decompression_requirement).
/// 2) Decompress the `IDAT` to an appropriately large buffer using
///    [`decompress_slice_iter_to_slice`](miniz_oxide::inflate::decompress_slice_iter_to_slice)
///    and passing it this iterator. You could use any other Zlib implementation
///    I guess, but the `miniz_oxide` crate is considered the "officially
///    supported" decompressor.
///
/// Those steps get you the *filtered* bytes of your PNG. You must still
/// unfilter, and possibly de-interlace, the data before it'll be useful!
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
pub const fn reduced_image_dimensions(full_width: u32, full_height: u32) -> [(u32, u32); 8] {
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

impl IHDR {
  /// Unfilters data from the zlib decompression buffer.
  ///
  /// ## Errors
  /// Possible errors include:
  /// * if `width` or `height` is 0.
  /// * if the `bit_depth` is illegal for the specified color type.
  pub fn unfilter_decompressed_data<F>(
    &self, mut decompressed: &mut [u8], mut out_op: F,
  ) -> Result<(), ()>
  where
    F: FnMut(u32, u32, &[u8]),
  {
    if self.width == 0 || self.height == 0 {
      return Err(());
    }

    // filtering is per byte within a pixel when pixels are more than 1 byte
    // each, and per byte when pixels are 1 byte or less.
    let filter_chunk_size = match self.color_type {
      PngColorType::Y => 1,
      PngColorType::RGB => match self.bit_depth {
        8 => 3,
        16 => 6,
        _ => return Err(()),
      },
      PngColorType::Index => 1,
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

    Err(())
  }
}
