use super::{
  chunk::{PngChunk, PngChunkTy},
  PngError, PngResult,
};

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
  pub fn from_ihdr_chunk(chunk: PngChunk<'_>) -> PngResult<Self> {
    if chunk.ty() != PngChunkTy::IHDR || chunk.data().len() != 13 {
      Err(PngError::NotAnIhdrChunk)
    } else {
      let data = chunk.data();
      let width = u32::from_be_bytes(data[0..4].try_into().unwrap());
      let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
      let bit_depth = data[8];
      let color_type = PngColorType(data[9]);
      let compression_method = PngCompressionMethod(data[10]);
      let filter_method = PngFilterMethod(data[11]);
      let interlace_method = PngInterlaceMethod(data[12]);
      Ok(Self {
        width,
        height,
        bit_depth,
        color_type,
        compression_method,
        filter_method,
        interlace_method,
      })
    }
  }

  /// Returns the number of bytes per unfiltering chunk.
  ///
  /// Depending on bit depth and channel count, this can be from 1 to 8.
  pub fn get_filtered_pixel_size(self) -> PngResult<usize> {
    Ok(match self.color_type {
      PngColorType::Y if [1, 2, 4, 8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          2
        } else {
          1
        }
      }
      PngColorType::INDEX if [1, 2, 4, 8].contains(&self.bit_depth) => 1,
      PngColorType::RGB if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          6
        } else {
          3
        }
      }
      PngColorType::YA if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          4
        } else {
          2
        }
      }
      PngColorType::RGBA if [8, 16].contains(&self.bit_depth) => {
        if self.bit_depth == 16 {
          8
        } else {
          4
        }
      }
      _ => return Err(PngError::IllegalColorTypeBitDepthCombination),
    })
  }

  /// The number of bytes in each filtered line of data.
  ///
  /// This is the bytes per scanline, +1 byte for the filter type.
  pub fn get_temp_memory_bytes_per_filterline(self) -> PngResult<usize> {
    if self.interlace_method == PngInterlaceMethod::NO_INTERLACE {
      let w = self.width as usize;
      if w == 0 {
        Err(PngError::IllegalWidthZero)
      } else {
        Ok(
          1 + match self.color_type {
            PngColorType::Y if [1, 2, 4, 8, 16].contains(&self.bit_depth) => {
              let bits_per_scanline =
                w.checked_mul(self.bit_depth as usize).ok_or(PngError::OutputOverflow)?;
              (bits_per_scanline + 7) / 8
            }
            PngColorType::INDEX if [1, 2, 4, 8].contains(&self.bit_depth) => {
              let bits_per_scanline =
                w.checked_mul(self.bit_depth as usize).ok_or(PngError::OutputOverflow)?;
              (bits_per_scanline + 7) / 8
            }
            PngColorType::RGB if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(3 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            PngColorType::YA if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(2 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            PngColorType::RGBA if [8, 16].contains(&self.bit_depth) => {
              w.checked_mul(4 * (self.bit_depth as usize / 8)).ok_or(PngError::OutputOverflow)?
            }
            _ => return Err(PngError::IllegalColorTypeBitDepthCombination),
          },
        )
      }
    } else {
      Err(PngError::InterlaceNotSupported)
    }
  }

  pub fn get_temp_memory_requirements(self) -> PngResult<usize> {
    if self.interlace_method == PngInterlaceMethod::NO_INTERLACE {
      let bytes_per_scanline: usize = self.get_temp_memory_bytes_per_filterline()?;
      let h = self.height as usize;
      if h == 0 {
        Err(PngError::IllegalHeightZero)
      } else {
        Ok(bytes_per_scanline * h)
      }
    } else {
      Err(PngError::InterlaceNotSupported)
    }
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
