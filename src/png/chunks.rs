use bytemuck::cast_slice;

use crate::pixel_formats::RGB8;

use super::*;

/// Enum for a fully parsed PNG chunk.
///
/// When you have a [`RawPngChunk`], use `PngChunk::try_from` to attempt to
/// convert it into this form.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum PngChunk<'b> {
  IHDR(IHDR),
  PLTE(PLTE<'b>),
  IDAT(IDAT<'b>),
  IEND,
  tRNS(tRNS<'b>),
  cHRM(cHRM),
  gAMA(gAMA),
  iCCP(iCCP<'b>),
  sBIT(sBIT),
  sRGB(sRGB),
  tEXt(tEXt<'b>),
  zTXt(zTXt<'b>),
  iTXt(iTXt<'b>),
  bKGD(bKGD),
  hIST(hIST<'b>),
  pHYs(pHYs),
  sPLT(sPLT<'b>),
  tIME(tIME),
}
impl<'b> PngChunk<'b> {
  pub fn to_ihdr(self) -> Option<IHDR> {
    match self {
      Self::IHDR(ihdr) => Some(ihdr),
      _ => None,
    }
  }
}
impl<'b> TryFrom<RawPngChunk<'b>> for PngChunk<'b> {
  type Error = PngError;
  #[must_use]
  fn try_from(
    RawPngChunk { chunk_ty, data, declared_crc: _ }: RawPngChunk<'b>,
  ) -> Result<Self, Self::Error> {
    use PngError::*;
    Ok(match &chunk_ty {
      b"IHDR" => {
        if data.len() != 13 || data[10] != 0 || data[11] != 0 || data[12] > 1 {
          return Err(Illegal_IHDR);
        }
        PngChunk::IHDR(IHDR {
          width: u32::from_be_bytes(data[0..4].try_into().unwrap()),
          height: u32::from_be_bytes(data[4..8].try_into().unwrap()),
          pixel_format: match (data[8], data[9]) {
            (1, 0) => PngPixelFormat::Y1,
            (2, 0) => PngPixelFormat::Y2,
            (4, 0) => PngPixelFormat::Y4,
            (8, 0) => PngPixelFormat::Y8,
            (16, 0) => PngPixelFormat::Y16,
            (8, 2) => PngPixelFormat::RGB8,
            (16, 2) => PngPixelFormat::RGB16,
            (1, 3) => PngPixelFormat::I1,
            (2, 3) => PngPixelFormat::I2,
            (4, 3) => PngPixelFormat::I4,
            (8, 3) => PngPixelFormat::I8,
            (8, 4) => PngPixelFormat::YA8,
            (16, 4) => PngPixelFormat::YA16,
            (8, 6) => PngPixelFormat::RGBA8,
            (16, 6) => PngPixelFormat::RGBA16,
            _ => return Err(Illegal_IHDR),
          },
          is_interlaced: data[12] == 1,
        })
      }
      b"PLTE" => {
        if (data.len() % 3) != 0 {
          return Err(Illegal_PLTE);
        }
        PngChunk::PLTE(PLTE { data: cast_slice(data) })
      }
      b"IDAT" => PngChunk::IDAT(IDAT { data }),
      b"IEND" => match *data {
        [] => PngChunk::IEND,
        _ => return Err(Illegal_IEND),
      },
      b"tRNS" => match *data {
        [y0, y1] => PngChunk::tRNS(tRNS::Y { y: u16::from_be_bytes([y0, y1]) }),
        [r0, r1, g0, g1, b0, b1] => PngChunk::tRNS(tRNS::RGB {
          r: u16::from_be_bytes([r0, r1]),
          g: u16::from_be_bytes([g0, g1]),
          b: u16::from_be_bytes([b0, b1]),
        }),
        _ => PngChunk::tRNS(tRNS::Index { data }),
      },
      b"cHRM" => {
        if data.len() != (4 * 8) {
          return Err(Illegal_cHRM);
        }
        PngChunk::cHRM(cHRM {
          white_x: u32::from_be_bytes(data[0..4].try_into().unwrap()),
          white_y: u32::from_be_bytes(data[4..8].try_into().unwrap()),
          red_x: u32::from_be_bytes(data[8..12].try_into().unwrap()),
          red_y: u32::from_be_bytes(data[12..16].try_into().unwrap()),
          green_x: u32::from_be_bytes(data[16..20].try_into().unwrap()),
          green_y: u32::from_be_bytes(data[20..24].try_into().unwrap()),
          blue_x: u32::from_be_bytes(data[24..28].try_into().unwrap()),
          blue_y: u32::from_be_bytes(data[28..32].try_into().unwrap()),
        })
      }
      b"gAMA" => {
        if data.len() != 4 {
          return Err(Illegal_gAMA);
        }
        PngChunk::gAMA(gAMA { gamma: u32::from_be_bytes(data.try_into().unwrap()) })
      }
      b"iCCP" => {
        let mut it = data.splitn(2, |u| u == &0_u8);
        let name = it.next().ok_or(Illegal_iCCP)?;
        match it.next().ok_or(Illegal_iCCP)? {
          [0, zlib_data @ ..] => PngChunk::iCCP(iCCP { name, zlib_data }),
          _ => return Err(Illegal_iCCP),
        }
      }
      b"sBIT" => match *data {
        [y] => PngChunk::sBIT(sBIT::Y { y }),
        [y, a] => PngChunk::sBIT(sBIT::YA { y, a }),
        [r, g, b] => PngChunk::sBIT(sBIT::RGB { r, g, b }),
        [r, g, b, a] => PngChunk::sBIT(sBIT::RGBA { r, g, b, a }),
        _ => return Err(Illegal_sBIT),
      },
      b"sRGB" => PngChunk::sRGB(sRGB {
        intent: match data {
          [0] => PngSrgbIntent::Perceptual,
          [1] => PngSrgbIntent::RelativeColorimetric,
          [2] => PngSrgbIntent::Saturation,
          [4] => PngSrgbIntent::AbsoluteColorimetric,
          _ => return Err(Illegal_sRGB),
        },
      }),
      b"tEXt" => {
        let mut it = data.splitn(2, |u| u == &0_u8);
        let keyword = it.next().ok_or(Illegal_tEXt)?;
        let text = it.next().ok_or(Illegal_tEXt)?;
        PngChunk::tEXt(tEXt { keyword, text })
      }
      b"zTXt" => {
        let mut it = data.splitn(2, |u| u == &0_u8);
        let keyword = it.next().ok_or(Illegal_zTXt)?;
        match it.next().ok_or(Illegal_zTXt)? {
          [0, zlib_data @ ..] => PngChunk::zTXt(zTXt { keyword, zlib_data }),
          _ => return Err(Illegal_zTXt),
        }
      }
      b"iTXt" => {
        let mut it = data.splitn(4, |u| u == &0_u8);
        let keyword = it.next().ok_or(Illegal_iTXt)?;
        // flag is 0 or 1, method should always be 0
        let flag_method_lang = it.next().ok_or(Illegal_iTXt)?;
        let translated_keyword =
          core::str::from_utf8(it.next().ok_or(Illegal_iTXt)?).map_err(|_| Illegal_iTXt)?;
        let text = it.next().ok_or(Illegal_iTXt)?;
        match flag_method_lang {
          [0, 0, lang @ ..] => PngChunk::iTXt(iTXt {
            keyword,
            lang,
            text,
            text_is_compressed: false,
            translated_keyword,
          }),
          [1, 0, lang @ ..] => PngChunk::iTXt(iTXt {
            keyword,
            lang,
            text,
            text_is_compressed: true,
            translated_keyword,
          }),
          _ => return Err(Illegal_iTXt),
        }
      }
      b"bKGD" => match *data {
        [i] => PngChunk::bKGD(bKGD::Index { i }),
        [y0, y1] => PngChunk::bKGD(bKGD::Y { y: u16::from_be_bytes([y0, y1]) }),
        [r0, r1, g0, g1, b0, b1] => PngChunk::bKGD(bKGD::RGB {
          r: u16::from_be_bytes([r0, r1]),
          g: u16::from_be_bytes([g0, g1]),
          b: u16::from_be_bytes([b0, b1]),
        }),
        _ => return Err(Illegal_bKGD),
      },
      b"hIST" => {
        if (data.len() % 2) == 0 {
          PngChunk::hIST(hIST { data: cast_slice(data) })
        } else {
          return Err(Illegal_hIST);
        }
      }
      b"pHYs" => {
        if data.len() != 9 || data[8] > 1 {
          return Err(Illegal_pHYs);
        }
        PngChunk::pHYs(pHYs {
          ppu_x: u32::from_be_bytes(data[0..4].try_into().unwrap()),
          ppu_y: u32::from_be_bytes(data[4..8].try_into().unwrap()),
          is_meters: data[8] != 0,
        })
      }
      b"sPLT" => {
        let mut it = data.splitn(2, |u| u == &0_u8);
        let palette_name = it.next().ok_or(Illegal_sPLT)?;
        match it.next().ok_or(Illegal_sPLT)? {
          [8, entries @ ..] => PngChunk::sPLT(sPLT { palette_name, is_16bit: false, entries }),
          [16, entries @ ..] => PngChunk::sPLT(sPLT { palette_name, is_16bit: true, entries }),
          _ => return Err(Illegal_sPLT),
        }
      }
      b"tIME" => match *data {
        [y0, y1, month, day, hour, minute, second] => PngChunk::tIME(tIME {
          year: u16::from_be_bytes([y0, y1]),
          month,
          day,
          hour,
          minute,
          second,
        }),
        _ => return Err(Illegal_tIME),
      },
      _ => return Err(UnknownChunkType),
    })
  }
}

/// The pixel formats allowed in a PNG file.
///
/// This combines a channel ordering with a bit depth per channel.
///
/// * The Greyscale (`Y`) and Indexed (`I`) formats allow for pixels that are
///   only 1, 2, or 4 bits each. In this case, the pixels are tightly packed
///   into bytes, with the left-most pixel being the highest bits of the byte.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum PngPixelFormat {
  Y1,
  Y2,
  Y4,
  Y8,
  Y16,
  RGB8,
  RGB16,
  I1,
  I2,
  I4,
  I8,
  YA8,
  YA16,
  RGBA8,
  RGBA16,
}
impl PngPixelFormat {
  /// Given an image's *pixel* width, calculates the *bytes* for a full scanline
  /// in this format.
  ///
  /// This doesn't include the filter byte for each line, so the temporary
  /// memory requirements will be slightly larger.
  #[inline]
  #[must_use]
  pub const fn bytes_per_scanline(self, width: u32) -> usize {
    let width = width as usize;
    match self {
      Self::Y1 | Self::I1 => width / 8 + if (width % 8) != 0 { 1 } else { 0 },
      Self::Y2 | Self::I2 => width / 4 + if (width % 4) != 0 { 1 } else { 0 },
      Self::Y4 | Self::I4 => width / 2 + if (width % 2) != 0 { 1 } else { 0 },
      Self::Y8 | Self::I8 => width,
      Self::Y16 => width * 1 * 2,
      Self::RGB8 => width * 3 * 1,
      Self::RGB16 => width * 3 * 2,
      Self::YA8 => width * 2 * 1,
      Self::YA16 => width * 2 * 2,
      Self::RGBA8 => width * 4 * 1,
      Self::RGBA16 => width * 4 * 2,
    }
  }
  #[inline]
  #[must_use]
  pub const fn bytes_per_pixel(self) -> usize {
    use PngPixelFormat::*;
    match self {
      Y1 | Y2 | Y4 | Y8 | I1 | I2 | I4 | I8 => 1,
      Y16 | YA8 => 2,
      RGB8 => 3,
      YA16 | RGBA8 => 4,
      RGB16 => 6,
      RGBA16 => 8,
    }
  }
  #[inline]
  #[must_use]
  pub const fn bits_per_channel(self) -> usize {
    use PngPixelFormat::*;
    match self {
      Y1 | I1 => 1,
      Y2 | I2 => 2,
      Y4 | I4 => 4,
      Y8 | I8 | YA8 | RGB8 | RGBA8 => 8,
      Y16 | RGB16 | YA16 | RGBA16 => 16,
    }
  }
}

/// Get the temp bytes for a given image.
///
/// * Interlaced images will have to call this function for all 7 reduced images
///   and then add up the values.
/// * Non-interlaced images call this function just once for their normal
///   dimensions.
#[inline]
#[must_use]
const fn temp_bytes_for_image(width: u32, height: u32, pixel_format: PngPixelFormat) -> usize {
  if width == 0 {
    return 0;
  }
  let bytes_per_scanline: usize = pixel_format.bytes_per_scanline(width);
  let bytes_per_filterline: usize = bytes_per_scanline.saturating_add(1);
  bytes_per_filterline.saturating_mul(height as usize)
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
pub const fn interlaced_pos_to_full_pos(
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

/// `IHDR`: Image header.
#[derive(Debug, Clone, Copy)]
pub struct IHDR {
  /// Width in pixels.
  ///
  /// Shouldn't be less than 1 or more than `i32::MAX`.
  pub width: u32,

  /// Height in pixels.
  ///
  /// Shouldn't be less than 1 or more than `i32::MAX`.
  pub height: u32,

  /// Format of the pixels.
  pub pixel_format: PngPixelFormat,

  /// If the pixel data is interlaced or not.
  pub is_interlaced: bool,
}
impl IHDR {
  /// Gets the temporary memory required to decompress the `IDAT` zlib data.
  ///
  /// Note: **currently** this is 1 byte more than the strict minimum
  /// requirement because of an issue with the decompression logic of the
  /// `miniz_oxide` crate. See
  /// [miniz_oxide#110](https://github.com/Frommi/miniz_oxide/issues/110).
  #[inline]
  #[must_use]
  pub fn temp_memory_requirement(self) -> usize {
    // TODO: remove the 1+ part when `miniz_oxide` is fixed
    1 + if self.is_interlaced {
      let mut total = 0;
      for (width, height) in reduced_image_dimensions(self.width, self.height) {
        total += temp_bytes_for_image(width, height, self.pixel_format);
      }
      total
    } else {
      temp_bytes_for_image(self.width, self.height, self.pixel_format)
    }
  }
}

/// `PLTE`: Palette.
///
/// * This chunk is required for indexed color PNGs, and it allows converting
///   index values to `RGBA8` values. There should be no more entries than the
///   bit depth of the index type, but there can be fewer entries. If an index
///   is out of range for the palette entries that's technically an error, but
///   probably you should use `pal.data.get(index).unwrap_or_default()` and just
///   have any error pixels be transparent black.
/// * This chunk is optional for RGB and RGBA color PNGs. If present with this
///   PNG type, it represents a suggested palette for how to quantize the image
///   if the display only supports limited colors.
/// * This chunk should not be present in Y or YA color PNGs.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct PLTE<'b> {
  pub data: &'b [RGB8],
}

/// `IDAT`: Image data.
///
/// One or more image data chunks hold a zlib data stream which should be
/// decompressed to get the filtered images of the PNG. If there's more than one
/// image data chunks in a PNG (the common case) then they should appear
/// directly one after the other in the PNG data stream.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct IDAT<'b> {
  pub data: &'b [u8],
}

/// `IEND`: Image trailer.
///
/// This should be the final chunk in a PNG, which lets you know that you have
/// the complete PNG data stream.
#[derive(Debug, Clone, Copy)]
pub struct IEND;

/// `tRNS`: Transparency.
///
/// Stores additional transparency data.
///
/// * `Y` and `RGB` each store a single color. All samples of that color in the
///   image are fully transparent (alpha 0), while the rest are fully opaque
///   (alpha maximum). The `tRNS` chunk always uses a `u16` to store the value,
///   even if the image's bit depth is less than 16.
/// * `Index` has an alpha value that goes along with the rest of the palette
///   data. The tranparency slice length should be less than or equal to the
///   palette slice length. If the transparency slice is shorter, all missing
///   entries should be assumed to have an alpha value of 255.
///
/// **Note:** The parser will pick `Y` or `RGB` based on the data length, so if
/// the image is indexed color and you see a `tRNS` chunk with the `Y` or `RGB`
/// variants that was *supposed* to be a slice of `Index` transparency info. Use
/// the `y_to_index` or `rgb_to_index` methods as appropriate.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum tRNS<'b> {
  Y { y: u16 },
  RGB { r: u16, g: u16, b: u16 },
  Index { data: &'b [u8] },
}
impl<'b> tRNS<'b> {
  /// Convert a `tRNS::Y` back to the index bytes.
  ///
  /// Gives `None` if this isn't the `Y` variant.
  #[inline]
  #[must_use]
  pub const fn y_to_index(self) -> Option<[u8; 2]> {
    match self {
      Self::Y { y } => Some(y.to_be_bytes()),
      _ => None,
    }
  }

  /// Convert a `tRNS::RGB` back to the index bytes.
  ///
  /// Gives `None` if this isn't the `RGB` variant.
  #[inline]
  #[must_use]
  pub const fn rgb_to_index(self) -> Option<[u8; 6]> {
    match self {
      Self::RGB { r, g, b } => {
        let [r0, r1] = r.to_be_bytes();
        let [g0, g1] = g.to_be_bytes();
        let [b0, b1] = b.to_be_bytes();
        Some([r0, r1, g0, g1, b0, b1])
      }
      _ => None,
    }
  }
}

/// `cHRM`: Primary chromaticities and white point.
///
/// Stores chromacity data.
///
/// Values are stored as an integer 100,000 the floating point value.
///
/// **Example:** A value of 0.3127 would be stored as the integer 31270.
///
/// An `sRGB` chunk or `iCCP` chunk, when present and recognized, overrides the
/// `cHRM` chunk.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct cHRM {
  pub white_x: u32,
  pub white_y: u32,
  pub red_x: u32,
  pub red_y: u32,
  pub green_x: u32,
  pub green_y: u32,
  pub blue_x: u32,
  pub blue_y: u32,
}
impl cHRM {
  pub const SRGB_CHROMACITY: Self = cHRM {
    white_x: 31_270,
    white_y: 32_900,
    red_x: 64_000,
    red_y: 33_000,
    green_x: 30_000,
    green_y: 60_000,
    blue_x: 15_000,
    blue_y: 6_000,
  };
}

/// `gAMA`: Image gamma.
///
/// Stores gamma data.
///
/// Values are stored as an integer 100,000 the floating point value.
///
/// **Example:** A gamma of 1/2.2 would be stored as the integer 45,455.
///
/// An `sRGB` chunk or `iCCP` chunk, when present and recognized, overrides the
/// `gAMA` chunk.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct gAMA {
  pub gamma: u32,
}
impl gAMA {
  pub const SRGB_GAMMA: Self = gAMA { gamma: 45_455 };
}

/// `iCCP`: Embedded ICC profile.
///
/// * The profile `name` may be any convenient name for referring to the
///   profile. It is case-sensitive. It is expected to contain Latin-1 text
///   without any null bytes.
/// * The `data` is a zlib data stream, and decompression of this datastream
///   yields the embedded ICC profile.
/// * If ICC profiles are supported by the decoder then use of this chunk (or
///   `SRGB`) should be preferred over the `gAMA` or `cHRM` chunks.
///
/// If this chunk is present, then the `sRGB` chunk *should not* be present.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct iCCP<'b> {
  pub name: &'b [u8],
  pub zlib_data: &'b [u8],
}

/// `sBIT`: Significant bits.
///
/// Gives the original number of significant bits per channel in the image. This
/// is only needed if the number of significant bits in the image is less than
/// the full bit depth of the image. For example, in an `RGB8` PNG, perhaps the
/// original image only had 5 significant bits of color depth to it.
///
/// * Each value here should be more than 0 and less than the full bit depth of
///   this PNG's pixel format.
/// * Indexed color uses the `RGB` variant, and the values must be less than 8.
/// * The variant used should match the color type of the image, possibly with
///   alpha added or removed.
///
/// The `sBIT` chunk might *not* have transparency information even if the image
/// data does (either because the color format has transparency, or because a
/// `tRNS` chunk is specified). In this case, all bits of transparency
/// information are assumed to have been significant.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum sBIT {
  Y { y: u8 },
  RGB { r: u8, g: u8, b: u8 },
  YA { y: u8, a: u8 },
  RGBA { r: u8, g: u8, b: u8, a: u8 },
}

/// Used by [`sRGB`] chunks.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum PngSrgbIntent {
  /// for images preferring good adaptation to the output device gamut at the
  /// expense of colorimetric accuracy, such as photographs.
  Perceptual = 0,
  /// for images requiring colour appearance matching (relative to the output
  /// device white point), such as logos.
  RelativeColorimetric = 1,
  /// for images preferring preservation of saturation at the expense of hue and
  /// lightness, such as charts and graphs.
  Saturation = 2,
  /// for images requiring preservation of absolute colorimetry, such as
  /// previews of images destined for a different output device (proofs).
  AbsoluteColorimetric = 4,
}

/// `sRGB`: Standard RGB colour space.
///
/// If the `sRGB` chunk is present, the image samples conform to the
/// [sRGB](https://en.wikipedia.org/wiki/SRGB) colour space.
///
/// The image should be displayed using the specified rendering `intent`, as
/// defined by the International Color Consortium.
///
/// If `sRGB` is present it overrides any `gAMA` and/or `cHRM`.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct sRGB {
  pub intent: PngSrgbIntent,
}

/// `tEXt`: Textual data.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct tEXt<'b> {
  pub keyword: &'b [u8],
  /// Should contain Latin-1 text.
  pub text: &'b [u8],
}

/// `zTXt`: Compressed textual data.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct zTXt<'b> {
  pub keyword: &'b [u8],
  pub zlib_data: &'b [u8],
}

/// `iTXt`: International textual data.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct iTXt<'b> {
  pub keyword: &'b [u8],
  pub text_is_compressed: bool,
  pub lang: &'b [u8],
  /// The keyword value, translated into the target language
  pub translated_keyword: &'b str,
  /// Possibly-compressed data, when in decompressed form it should be UTF-8
  /// text in the target language.
  pub text: &'b [u8],
}

/// `bKGD`: Background colour.
///
/// Gives an intended background color for the image.
///
/// The color type should match the color type of the image. The implied alpha
/// value is always "fully opaque" (eg: 255 for 8-bit alpha).
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum bKGD {
  Y { y: u16 },
  RGB { r: u16, g: u16, b: u16 },
  Index { i: u8 },
}

/// `hIST`: Image Histogram.
///
/// Gives the approximate usage frequency of each color in the palette.
///
/// Can appear only when a `PLTE` chunk appears. If a viewer is unable to
/// provide all the colours listed in the palette, the histogram may help it
/// decide how to choose a subset of the colours for display.
///
/// * There shall be exactly one entry for each entry in the `PLTE` chunk. Each
///   entry is proportional to the fraction of pixels in the image that have
///   that palette index; the exact scale factor is chosen by the encoder.
/// * Histogram entries are approximate, with the exception that a zero entry
///   specifies that the corresponding palette entry is not used at all in the
///   image. A histogram entry shall be nonzero if there are any pixels of that
///   colour.
///
/// Histogram data in this struct is stored as 2-byte big-endian values. It's
/// given as a slice to avoid allocation, because the data length is dynamic
/// (the length should match the length of the palette).
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct hIST<'b> {
  pub data: &'b [[u8; 2]],
}

/// `pHYs`: Physical pixel dimensions.
///
/// Specifies the intended pixel size or aspect ratio for display of the image.
///
/// When `is_meters` is set then `x` and `y` are in pixels per meter.
/// Otherwise they have no unit and define an aspect ratio only.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct pHYs {
  pub ppu_x: u32,
  pub ppu_y: u32,
  pub is_meters: bool,
}

/// `sPLT`: Suggested palette data.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct sPLT<'b> {
  pub palette_name: &'b [u8],
  pub is_16bit: bool,
  pub entries: &'b [u8],
}

/// `tIME`: Image last-modification time.
///
/// Last image modification time, UTC.
#[derive(Debug, Clone, Copy)]
pub struct tIME {
  /// 4-digit year.
  pub year: u16,
  /// 1-12
  pub month: u8,
  /// 1-31
  pub day: u8,
  /// 0-23
  pub hour: u8,
  /// 0-59
  pub minute: u8,
  /// 0-60 (use 60 for leap seconds)
  pub second: u8,
}

/// A raw portion of a PNG datastream.
///
/// The header for a chunk gives its type and data length. After the data
/// there's a CRC for the chunk. This type captures that info, without
/// attempting to examine if the data portion fits with the chunk type.
///
/// Generally you'll convert this into a [`PngChunk`] using `try_from`.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct RawPngChunk<'b> {
  pub chunk_ty: [u8; 4],
  pub data: &'b [u8],
  pub declared_crc: u32,
}

/// An iterator over the raw chunks of a PNG.
#[derive(Clone)]
pub struct RawPngChunkIter<'b> {
  spare: &'b [u8],
}
impl<'b> RawPngChunkIter<'b> {
  /// Makes an iterator over a PNG's chunks.
  ///
  /// ## Failure
  /// This function always returns an iterator. However, if the slice doesn't
  /// start with the correct PNG signature then an empty slice will be stored,
  /// and the first call to `next` will end up returning `None`.
  #[inline]
  #[must_use]
  pub const fn new(png: &'b [u8]) -> Self {
    Self {
      spare: match png {
        [137, 80, 78, 71, 13, 10, 26, 10, spare @ ..] => spare,
        _ => &[],
      },
    }
  }
}
impl<'b> Iterator for RawPngChunkIter<'b> {
  type Item = RawPngChunk<'b>;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      None
    } else {
      let (len, rest) = if self.spare.len() < 4 {
        self.spare = &[];
        return None;
      } else {
        let (len_bytes, rest) = self.spare.split_at(4);
        let len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
        (len, rest)
      };
      let (chunk_ty, rest) = if rest.len() < 4 {
        self.spare = &[];
        return None;
      } else {
        let (ty_bytes, rest) = rest.split_at(4);
        (ty_bytes.try_into().unwrap(), rest)
      };
      let (data, rest) = if rest.len() < len {
        self.spare = &[];
        return None;
      } else {
        rest.split_at(len)
      };
      let (declared_crc, rest) = if rest.len() < 4 {
        self.spare = &[];
        return None;
      } else {
        let (decl_crc_bytes, rest) = rest.split_at(4);
        (u32::from_be_bytes(decl_crc_bytes.try_into().unwrap()), rest)
      };
      self.spare = rest;
      Some(RawPngChunk { chunk_ty, data, declared_crc })
    }
  }
}
