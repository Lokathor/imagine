
use super::*;

#[derive(Debug, Clone, Copy)]
pub enum BmpDataFormat {
  Indexed1 {
    palette_span: (usize, usize),
  },
  Indexed4 {
    palette_span: (usize, usize),
  },
  Indexed4Rle {
    palette_span: (usize, usize),
  },
  Indexed8 {
    palette_span: (usize, usize),
  },
  Indexed8Rle {
    palette_span: (usize, usize),
  },
  Bitmask16RGB {
    r_mask: u16,
    g_mask: u16,
    b_mask: u16,
  },
  Bitmask16RGBA {
    r_mask: u16,
    g_mask: u16,
    b_mask: u16,
    a_mask: u16,
  },
  BGR24,
  Bitmask32RGB {
    r_mask: u32,
    g_mask: u32,
    b_mask: u32,
  },
  Bitmask32RGBA {
    r_mask: u32,
    g_mask: u32,
    b_mask: u32,
    a_mask: u32,
  },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BmpHeader {
  pub width: u32,
  pub height: u32,
  pub origin_top_left: bool,
  pub data_format: BmpDataFormat,
  pub data_span: (usize, usize),
}
impl TryFrom<BitmapV5Header> for BmpHeader {
  type Error = ImagineError;
  fn try_from(v5: BitmapV5Header) -> Result<Self, Self::Error> {
    let width = v5.width.get().unsigned_abs();
    let height = v5.height.get().unsigned_abs();
    let origin_top_left = v5.width.get.is_negative();
    todo!();
    Ok(Self {
      width,
      height,
      origin_top_left,
      data_format,
      data_span,
    })
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
