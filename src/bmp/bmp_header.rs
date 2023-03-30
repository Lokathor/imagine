use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct BmpHeader {
  pub width: u32,
  pub height: u32,
  pub origin_top_left: bool,
  pub bits_per_pixel: u16,
  pub compression: Option<BmpCompression>,
  pub palette_span: Option<(usize, usize)>,
  pub image_span: (usize, usize),
}
impl BmpHeader {
  /// If the image is using the alpha channel.
  #[inline]
  #[must_use]
  pub const fn uses_alpha(&self) -> bool {
    self.bits_per_pixel == 32
      || matches!(self.compression, Some(BmpCompression::AlphaBitfields { .. }))
  }

  /// This gets the palette entries from the bytes.
  ///
  /// * The entries are `[b, g, r, _]`, or `[b, g, r, a]` if the image uses
  ///   alpha.
  /// * If the image uses alpha and all alpha values are 0, then instead you
  ///   should treat all alpha values as 255.
  #[inline]
  pub fn get_palette<'b>(&self, bytes: &'b [u8]) -> Result<&'b [[u8; 4]], ImagineError> {
    self
      .palette_span
      .and_then(|(low, high)| {
        if low < high && high <= bytes.len() {
          try_cast_slice(&bytes[low..high]).ok()
        } else {
          None
        }
      })
      .ok_or(ImagineError::Value)
  }

  /// Gets the bytes of the image data.
  #[inline]
  pub fn get_image_bytes<'b>(&self, bytes: &'b [u8]) -> Result<&'b [u8], ImagineError> {
    let (low, high) = self.image_span;
    if low < high && high <= bytes.len() {
      Ok(&bytes[low..high])
    } else {
      Err(ImagineError::Value)
    }
  }

  /// Runs the `(x,y,index)` op for all pixels.
  ///
  /// The run-length encoding compression can cause pixels to be handled out of
  /// order, and so the operation is always given the `(x,y)` affected.
  ///
  /// ## Failure
  /// * The bit depth and compression combination must be one of:
  ///   * 1, 2, 4, or 8 with no compression
  ///   * 4 or 8 with `RunLengthEncoding` compression
  #[inline]
  pub fn for_each_pal_index<F: FnMut(u32, u32, u8)>(
    &self, bytes: &[u8], mut op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      width,
      height,
      bits_per_pixel,
      compression,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match bits_per_pixel {
      1 | 2 | 4 | 8 if compression.is_none() => {
        let index_iter = bmp_iter_pal_indexes_no_compression(
          image_bytes,
          usize::from(*bits_per_pixel),
          (*width).try_into().unwrap(),
        );
        (0..*height)
          .flat_map(|y| (0..*width).map(move |x| (x, y)))
          .zip(index_iter)
          .for_each(|((x, y), p)| op(x, y, p))
      }
      4 if *compression == Some(BmpCompression::RunLengthEncoding) => {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        for rle_op in bmp_iter_rle4(image_bytes) {
          match rle_op {
            BmpRle4Op::EndOfBmp => return Ok(()),
            BmpRle4Op::Newline => {
              x = 0;
              y = y.wrapping_add(1);
            }
            BmpRle4Op::Run { count, index_h, index_l } => {
              let mut it = [index_h, index_l].into_iter().cycle();
              for _ in 0..count.get() {
                op(x, y, it.next().unwrap());
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Delta { right, up } => {
              x = x.wrapping_add(right);
              y = y.wrapping_add(up);
            }
            BmpRle4Op::Raw4 { a, b, c, d } => {
              for val in [a, b, c, d] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw3 { a, b, c } => {
              for val in [a, b, c] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw2 { a, b } => {
              for val in [a, b] {
                op(x, y, val);
                x = x.wrapping_add(1);
              }
            }
            BmpRle4Op::Raw1 { a } => {
              op(x, y, a);
              x = x.wrapping_add(1);
            }
          }
        }
      }
      8 if *compression == Some(BmpCompression::RunLengthEncoding) => {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        for rle_op in bmp_iter_rle8(image_bytes) {
          match rle_op {
            BmpRle8Op::EndOfBmp => return Ok(()),
            BmpRle8Op::Newline => {
              x = 0;
              y = y.wrapping_add(1);
            }
            BmpRle8Op::Run { count, index } => {
              for _ in 0..count.get() {
                op(x, y, index);
                x = x.wrapping_add(1);
              }
            }
            BmpRle8Op::Delta { right, up } => {
              x = x.wrapping_add(right);
              y = y.wrapping_add(up);
            }
            BmpRle8Op::Raw2 { q, w } => {
              op(x, y, q);
              x = x.wrapping_add(1);
              op(x, y, w);
              x = x.wrapping_add(1);
            }
            BmpRle8Op::Raw1 { q } => {
              op(x, y, q);
              x = x.wrapping_add(1);
            }
          }
        }
      }
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }

  /// Runs the op for all pixels
  ///
  /// Pixels proceed left to right across each scan line. Depending on the
  /// `origin_top_left` value in the header the scanlines proceed top down, or
  /// bottom up.
  ///
  /// ## Failure
  /// * The bit depth and compression combination must be one of:
  ///   * 24 with no compression
  ///   * 16 or 32, and `Bitfields` compression
  #[inline]
  pub fn for_each_rgb<F: FnMut(r32g32b32_Sfloat)>(
    &self, bytes: &[u8], op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      width,
      height: _,
      bits_per_pixel,
      compression,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match (bits_per_pixel, compression) {
      (24, None) => {
        let bgr_it = bmp_iter_bgr24(image_bytes, (*width).try_into().unwrap());
        bgr_it.map(|[b, g, r]| r32g32b32_Sfloat::from(r8g8b8_Srgb { r, g, b })).for_each(op)
      }
      (16, Some(BmpCompression::Bitfields { r_mask, g_mask, b_mask })) => {
        bmp_iter_bitmask16_rgb(image_bytes, *r_mask, *g_mask, *b_mask, (*width).try_into().unwrap())
          .for_each(op)
      }
      (32, Some(BmpCompression::Bitfields { r_mask, g_mask, b_mask })) => bmp_iter_bitmask32_srgb(
        image_bytes,
        *r_mask,
        *g_mask,
        *b_mask,
        (*width).try_into().unwrap(),
      )
      .map(r32g32b32_Sfloat::from)
      .for_each(op),
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }

  /// Runs the op for all pixels
  ///
  /// Pixels proceed left to right across each scan line. Depending on the
  /// `origin_top_left` value in the header the scanlines proceed top down, or
  /// bottom up.
  ///
  /// ## Failure
  /// * The bit depth must be 16 or 32, and the compression must be
  ///   `AlphaBitfields`
  #[inline]
  pub fn for_each_rgba<F: FnMut(r32g32b32a32_Sfloat)>(
    &self, bytes: &[u8], op: F,
  ) -> Result<(), ImagineError> {
    let Self {
      bits_per_pixel,
      compression,
      width,
      height: _,
      origin_top_left: _,
      palette_span: _,
      image_span: _,
    } = self;
    let image_bytes = self.get_image_bytes(bytes)?;
    match (bits_per_pixel, compression) {
      (16, Some(BmpCompression::AlphaBitfields { r_mask, g_mask, b_mask, a_mask })) => {
        bmp_iter_bitmask16_rgba(
          image_bytes,
          *r_mask,
          *g_mask,
          *b_mask,
          *a_mask,
          (*width).try_into().unwrap(),
        )
        .for_each(op)
      }
      (32, Some(BmpCompression::AlphaBitfields { r_mask, g_mask, b_mask, a_mask })) => {
        bmp_iter_bitmask32_srgba(
          image_bytes,
          *r_mask,
          *g_mask,
          *b_mask,
          *a_mask,
          (*width).try_into().unwrap(),
        )
        .map(r32g32b32a32_Sfloat::from)
        .for_each(op)
      }
      _ => return Err(ImagineError::Value),
    }
    Ok(())
  }
}

fn padded_bytes_per_line(width: u32, bits_per_pixel: u16) -> Result<usize, ImagineError> {
  let width: usize = width.try_into()?;
  let bits_per_pixel = usize::from(bits_per_pixel);
  let bits_per_line = bits_per_pixel.checked_mul(width).ok_or(ImagineError::Value)?;
  let bytes_per_line = bits_per_line / 8 + usize::from(bits_per_line % 8 != 0);
  let dwords_per_line = bytes_per_line / 4 + usize::from(bytes_per_line % 4 != 0);
  dwords_per_line.checked_mul(width).ok_or(ImagineError::Value)
}

#[inline]
#[allow(bad_style)]
pub fn bmp_get_header(bytes: &[u8]) -> Result<BmpHeader, ImagineError> {
  const size_BitmapCoreHeader: usize = size_of::<BitmapCoreHeader>();
  const size_BitmapInfoHeader: usize = size_of::<BitmapInfoHeader>();
  const size_BitmapV2InfoHeader: usize = size_of::<BitmapV2InfoHeader>();
  const size_BitmapV3InfoHeader: usize = size_of::<BitmapV3InfoHeader>();
  const size_BitmapV4Header: usize = size_of::<BitmapV4Header>();
  const size_BitmapV5Header: usize = size_of::<BitmapV5Header>();
  //
  let (file_header, rest) = try_pull_pod::<BitmapFileHeader>(bytes)?;
  let (info_header_size, _) = try_pull_pod::<U32LE>(rest)?;
  let (mut v5, _rest) = match info_header_size.get() as usize {
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
  dbg!(v5);
  Err(ImagineError::IncompleteLibrary)
}
