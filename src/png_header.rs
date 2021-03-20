use super::*;

#[derive(Debug, Clone, Copy)]
pub struct PngHeader {
  pub width: u32,
  pub height: u32,
  pub bit_depth: u8,
  pub color_type: PngColorType,
  pub compression_method: PngCompressionMethod,
  pub filter_method: PngFilterMethod,
  pub interlace_method: PngInterlaceMethod,
}
impl PngHeader {
  pub fn from_ihdr_chunk(chunk: PngChunk<'_>) -> Option<Self> {
    if chunk.chunk_type != ChunkType::IHDR || chunk.length != 13 {
      return None;
    }
    let width = u32::from_be_bytes(chunk.chunk_data[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(chunk.chunk_data[4..8].try_into().unwrap());
    let bit_depth = chunk.chunk_data[8];
    let color_type = PngColorType(chunk.chunk_data[9]);
    let compression_method = PngCompressionMethod(chunk.chunk_data[10]);
    let filter_method = PngFilterMethod(chunk.chunk_data[11]);
    let interlace_method = PngInterlaceMethod(chunk.chunk_data[12]);
    Some(Self {
      width,
      height,
      bit_depth,
      color_type,
      compression_method,
      filter_method,
      interlace_method,
    })
  }

  pub fn get_temp_memory_requirements(self) -> Option<usize> {
    if self.interlace_method != PngInterlaceMethod::NO_INTERLACE {
      return None;
    }
    let w = self.width as usize;
    let h = self.height as usize;
    let bytes_per_scanline: usize = match self.color_type {
      PngColorType::Y if [1, 2, 4, 8, 16].contains(&self.bit_depth) => {
        let bits_per_scanline = w.checked_mul(self.bit_depth as usize)?;
        (bits_per_scanline + 7) / 8
      }
      PngColorType::INDEX if [1, 2, 4, 8].contains(&self.bit_depth) => {
        let bits_per_scanline = w.checked_mul(self.bit_depth as usize)?;
        (bits_per_scanline + 7) / 8
      }
      PngColorType::RGB if [8, 16].contains(&self.bit_depth) => {
        w.checked_mul(3 * (self.bit_depth as usize / 8))?
      }
      PngColorType::YA if [8, 16].contains(&self.bit_depth) => {
        w.checked_mul(2 * (self.bit_depth as usize / 8))?
      }
      PngColorType::RGBA if [8, 16].contains(&self.bit_depth) => {
        w.checked_mul(4 * (self.bit_depth as usize / 8))?
      }
      _ => return None,
    };
    Some((bytes_per_scanline + 1) * h)
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngColorType(u8);
impl PngColorType {
  pub const Y: Self = Self(0);
  pub const RGB: Self = Self(2);
  pub const INDEX: Self = Self(3);
  pub const YA: Self = Self(4);
  pub const RGBA: Self = Self(6);
}
impl core::fmt::Debug for PngColorType {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngColorType::Y => write!(f, "Y"),
      PngColorType::RGB => write!(f, "RGB"),
      PngColorType::INDEX => write!(f, "Index"),
      PngColorType::YA => write!(f, "YA"),
      PngColorType::RGBA => write!(f, "RGBA"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngCompressionMethod(u8);
impl PngCompressionMethod {
  pub const DEFLATE: Self = Self(0);
}
impl core::fmt::Debug for PngCompressionMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngCompressionMethod::DEFLATE => write!(f, "Deflate"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngFilterMethod(u8);
impl PngFilterMethod {
  pub const ADAPTIVE: Self = Self(0);
}
impl core::fmt::Debug for PngFilterMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngFilterMethod::ADAPTIVE => write!(f, "Adaptive"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PngInterlaceMethod(u8);
impl PngInterlaceMethod {
  pub const NO_INTERLACE: Self = Self(0);
  pub const ADAM7: Self = Self(1);
}
impl core::fmt::Debug for PngInterlaceMethod {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      PngInterlaceMethod::NO_INTERLACE => write!(f, "NoInterlace"),
      PngInterlaceMethod::ADAM7 => write!(f, "Adam7"),
      other => write!(f, "Illegal({})", other.0),
    }
  }
}
