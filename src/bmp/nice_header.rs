use super::*;

#[derive(Debug, Clone, Copy)]
pub enum BmpDataFormat {
  Indexed1 { palette_span: (usize, usize) },
  Indexed4 { palette_span: (usize, usize) },
  Indexed4Rle { palette_span: (usize, usize) },
  Indexed8 { palette_span: (usize, usize) },
  Indexed8Rle { palette_span: (usize, usize) },
  Bitmask16RGB { r_mask: u16, g_mask: u16, b_mask: u16 },
  Bitmask16RGBA { r_mask: u16, g_mask: u16, b_mask: u16, a_mask: u16 },
  BGR24,
  Bitmask32RGB { r_mask: u32, g_mask: u32, b_mask: u32 },
  Bitmask32RGBA { r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32 },
}
impl BmpDataFormat {
  /// If the format is a run-length encoded format.
  #[inline]
  #[must_use]
  pub const fn is_rle(self) -> bool {
    matches!(self, Self::Indexed4Rle { .. } | Self::Indexed8Rle { .. })
  }
}

/// This is a nice, easy to use form of BMP header.
///
/// It collects the important info, and discards all the rest of the stuff you
/// don't need.
#[derive(Debug, Clone, Copy)]
pub struct BmpNiceHeader {
  pub width: u32,
  pub height: u32,
  pub origin_top_left: bool,
  pub data_format: BmpDataFormat,
  pub data_span: (usize, usize),
}

#[inline]
#[allow(bad_style)]
pub fn bmp_get_nice_header(bytes: &[u8]) -> Result<BmpNiceHeader, ImagineError> {
  const size_BitmapCoreHeader: usize = size_of::<BitmapCoreHeader>();
  const size_BitmapInfoHeader: usize = size_of::<BitmapInfoHeader>();
  const size_BitmapV2InfoHeader: usize = size_of::<BitmapV2InfoHeader>();
  const size_BitmapV3InfoHeader: usize = size_of::<BitmapV3InfoHeader>();
  const size_BitmapV4Header: usize = size_of::<BitmapV4Header>();
  const size_BitmapV5Header: usize = size_of::<BitmapV5Header>();
  //
  let (file_header, rest) = try_pull_pod::<BitmapFileHeader>(bytes)?;
  let (info_header_size, _) = try_pull_pod::<U32LE>(rest)?;
  // We "normalize" all headers into looking like a v5 header, and then write the
  // conversion to the nice header format just once.
  let (v5, _rest) = match usize::try_from(info_header_size.get())? {
    size_BitmapCoreHeader => {
      let (info, rest) = try_pull_pod::<BitmapCoreHeader>(rest)?;
      (BitmapV5Header::from(info), rest)
    }
    size_BitmapInfoHeader => {
      let (info, rest) = try_pull_pod::<BitmapInfoHeader>(rest)?;
      let mut v5 = BitmapV5Header::from(info);
      match v5.compression.get() {
        BI_BITFIELDS => {
          let ([r_mask, g_mask, b_mask], _rest) = try_pull_pod::<[U32LE; 3]>(rest)?;
          v5.r_mask = r_mask;
          v5.g_mask = g_mask;
          v5.b_mask = b_mask;
        }
        BI_ALPHABITFIELDS => {
          let ([r_mask, g_mask, b_mask, a_mask], _rest) = try_pull_pod::<[U32LE; 4]>(rest)?;
          v5.r_mask = r_mask;
          v5.g_mask = g_mask;
          v5.b_mask = b_mask;
          v5.a_mask = a_mask;
        }
        _ => (),
      }
      (v5, rest)
    }
    size_BitmapV2InfoHeader => {
      let (info, rest) = try_pull_pod::<BitmapV2InfoHeader>(rest)?;
      (BitmapV5Header::from(info), rest)
    }
    size_BitmapV3InfoHeader => {
      let (info, rest) = try_pull_pod::<BitmapV3InfoHeader>(rest)?;
      (BitmapV5Header::from(info), rest)
    }
    size_BitmapV4Header => {
      let (info, rest) = try_pull_pod::<BitmapV4Header>(rest)?;
      (BitmapV5Header::from(info), rest)
    }
    size_BitmapV5Header => try_pull_pod::<BitmapV5Header>(rest)?,
    _ => return Err(ImagineError::Parse),
  };
  //dbg!(v5);
  let width = v5.width.get().unsigned_abs();
  if width == 0 {
    return Err(ImagineError::Value);
  }
  let height = v5.height.get().unsigned_abs();
  if height == 0 {
    return Err(ImagineError::Value);
  }
  let origin_top_left = v5.height.get().is_negative();
  let data_format = {
    let bits_per_pixel = v5.bits_per_pixel.get();
    let compression = v5.compression.get();
    let pal_start: usize = size_of::<BitmapFileHeader>()
      .checked_add(info_header_size.get().try_into()?)
      .ok_or(ImagineError::Value)?;
    let pal_entry_count: usize = if v5.colors_used.get() == 0 {
      1_usize.wrapping_shl(u32::from(bits_per_pixel))
    } else {
      v5.colors_used.get().try_into()?
    };
    let pal_end: usize = pal_start
      .checked_add(pal_entry_count.checked_mul(4).ok_or(ImagineError::Value)?)
      .ok_or(ImagineError::Value)?;
    match (bits_per_pixel, compression) {
      (1, BI_RGB) => BmpDataFormat::Indexed1 { palette_span: (pal_start, pal_end) },
      (4, BI_RGB) => BmpDataFormat::Indexed4 { palette_span: (pal_start, pal_end) },
      (4, BI_RLE4) => BmpDataFormat::Indexed4Rle { palette_span: (pal_start, pal_end) },
      (8, BI_RGB) => BmpDataFormat::Indexed8 { palette_span: (pal_start, pal_end) },
      (8, BI_RLE8) => BmpDataFormat::Indexed8Rle { palette_span: (pal_start, pal_end) },
      (16, BI_RGB) => {
        BmpDataFormat::Bitmask16RGB { r_mask: 0b11111 << 10, g_mask: 0b11111 << 5, b_mask: 0b11111 }
      }
      (16, BI_BITFIELDS) | (16, BI_ALPHABITFIELDS) => {
        if v5.a_mask.get() != 0 {
          BmpDataFormat::Bitmask16RGBA {
            r_mask: v5.r_mask.get().try_into()?,
            g_mask: v5.g_mask.get().try_into()?,
            b_mask: v5.b_mask.get().try_into()?,
            a_mask: v5.a_mask.get().try_into()?,
          }
        } else {
          BmpDataFormat::Bitmask16RGB {
            r_mask: v5.r_mask.get().try_into()?,
            g_mask: v5.g_mask.get().try_into()?,
            b_mask: v5.b_mask.get().try_into()?,
          }
        }
      }
      (24, BI_RGB) => BmpDataFormat::BGR24,
      (32, BI_BITFIELDS) | (32, BI_ALPHABITFIELDS) => {
        if v5.a_mask.get() != 0 {
          BmpDataFormat::Bitmask32RGBA {
            r_mask: v5.r_mask.get(),
            g_mask: v5.g_mask.get(),
            b_mask: v5.b_mask.get(),
            a_mask: v5.a_mask.get(),
          }
        } else {
          BmpDataFormat::Bitmask32RGB {
            r_mask: v5.r_mask.get(),
            g_mask: v5.g_mask.get(),
            b_mask: v5.b_mask.get(),
          }
        }
      }
      _ => return Err(ImagineError::Value),
    }
  };
  let data_span = {
    let data_start: usize = file_header.bitmap_offset.get().try_into()?;
    let data_end: usize = if data_format.is_rle() {
      // The RLE encoding actually tells us when to stop, so we can just *pretend*
      // that the data goes all the way to the end of the file, rounded down to a byte
      // pair, and the RLE decode will work fine.
      (bytes.len() / 2) * 2
    } else {
      padded_bytes_per_line(width, v5.bits_per_pixel.get())?
        .checked_mul(height.try_into()?)
        .and_then(|count| data_start.checked_add(count))
        .ok_or(ImagineError::Value)?
    };
    (data_start, data_end)
  };
  let header = BmpNiceHeader { width, height, origin_top_left, data_format, data_span };
  //dbg!(header);
  Ok(header)
}
