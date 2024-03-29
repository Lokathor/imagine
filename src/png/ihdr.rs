use bitfrob::{u8_bit_split_1x8_rev, u8_bit_split_2x4_rev, u8_bit_split_4x2_rev};

use super::*;

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
  #[inline]
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
  #[inline]
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
  #[inline]
  pub const fn bytes_per_filterline(&self, width: u32) -> usize {
    // each line is a filter byte (1) + pixel data. When pixels are less than 8
    // bits per channel it's possible to end up with partial bytes on the end,
    // so we must round up.
    1 + ((self.bits_per_pixel() * (width as usize)) + 7) / 8
  }

  /// Gets the buffer size required to perform Zlib decompression.
  #[inline]
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
      for (width, height) in reduced_image_dimensions(self.width, self.height).into_iter().skip(1) {
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
  #[inline]
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
  #[inline]
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

impl IHDR {
  fn send_out_pixel<F: FnMut(u32, u32, &[u8])>(
    &self, image_level: usize, reduced_x: u32, reduced_y: u32, data: &[u8], op: &mut F,
  ) {
    let full_width = self.width;
    match self.bit_depth {
      1 => {
        for (plus_x, pix) in u8_bit_split_1x8_rev(data[0]).into_iter().enumerate() {
          let (image_x, image_y): (u32, u32) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 8 + (plus_x as u32), reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x, image_y, &[pix]);
        }
      }
      2 => {
        for (plus_x, pix) in u8_bit_split_2x4_rev(data[0]).into_iter().enumerate() {
          let (image_x, image_y) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 4 + (plus_x as u32), reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x, image_y, &[pix]);
        }
      }
      4 => {
        for (plus_x, pix) in u8_bit_split_4x2_rev(data[0]).into_iter().enumerate() {
          let (image_x, image_y) =
            interlaced_pos_to_full_pos(image_level, reduced_x * 2 + (plus_x as u32), reduced_y);
          if image_x >= full_width {
            // if we've gone outside the image's bounds then we're looking at
            // padding bits and we cancel the rest of the outputs in this
            // call of the function.
            return;
          }
          op(image_x, image_y, &[pix]);
        }
      }
      8 | 16 => {
        let (image_x, image_y) = interlaced_pos_to_full_pos(image_level, reduced_x, reduced_y);
        op(image_x, image_y, data);
      }
      _ => unreachable!(),
    }
  }

  /// Unfilters data from the zlib decompression buffer into the final
  /// destination.
  ///
  /// See the [`png` module docs](crate::png) for guidance.
  #[allow(clippy::result_unit_err)]
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn unfilter_decompressed_data<F>(
    &self, mut decompressed: &mut [u8], mut op: F,
  ) -> Result<(), ()>
  where
    F: FnMut(u32, u32, &[u8]),
  {
    if self.width == 0 || self.height == 0 {
      return Err(());
    }

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

      let (these_bytes, bytes_per_filterline) = if decompressed.len() < bytes_used_this_image {
        return Err(());
      } else {
        let (these_bytes, more_bytes) = decompressed.split_at_mut(bytes_used_this_image);
        decompressed = more_bytes;
        (these_bytes, bytes_per_filterline)
      };

      // first just unfilter in place.
      let row_iter = these_bytes.chunks_exact_mut(bytes_per_filterline);
      match self.color_type {
        PngColorType::Y => match self.bit_depth {
          16 => png_filters::unfilter_lines::<2>(row_iter),
          8 | 4 | 2 | 1 => png_filters::unfilter_lines::<1>(row_iter),
          _ => return Err(()),
        },
        PngColorType::RGB => match self.bit_depth {
          8 => png_filters::unfilter_lines::<3>(row_iter),
          16 => png_filters::unfilter_lines::<6>(row_iter),
          _ => return Err(()),
        },
        PngColorType::Index => match self.bit_depth {
          8 | 4 | 2 | 1 => png_filters::unfilter_lines::<1>(row_iter),
          _ => return Err(()),
        },
        PngColorType::YA => match self.bit_depth {
          8 => png_filters::unfilter_lines::<2>(row_iter),
          16 => png_filters::unfilter_lines::<4>(row_iter),
          _ => return Err(()),
        },
        PngColorType::RGBA => match self.bit_depth {
          8 => png_filters::unfilter_lines::<4>(row_iter),
          16 => png_filters::unfilter_lines::<8>(row_iter),
          _ => return Err(()),
        },
      };

      // then place all the pixels.
      let filter_chunk_size = match self.color_type {
        PngColorType::Y => match self.bit_depth {
          16 => 2,
          8 | 4 | 2 | 1 => 1,
          _ => return Err(()),
        },
        PngColorType::RGB => match self.bit_depth {
          8 => 3,
          16 => 6,
          _ => return Err(()),
        },
        PngColorType::Index => match self.bit_depth {
          8 | 4 | 2 | 1 => 1,
          _ => return Err(()),
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
      these_bytes
        .chunks_exact(bytes_per_filterline)
        .enumerate()
        .flat_map(|(y, line)| {
          let (_, line) = line.split_first().unwrap();
          line.chunks_exact(filter_chunk_size).enumerate().map(move |(x, px)| (x, y, px))
        })
        .for_each(|(reduced_, reduced_y, data)| {
          self.send_out_pixel(image_level, reduced_ as _, reduced_y as _, data, &mut op)
        });
    }

    //
    Ok(())
  }
}
