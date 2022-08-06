use core::fmt::{Debug, Write};

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
pub struct IHDR {
  width: u32,
  height: u32,
  bit_depth: u8,
  color_type: u8,
  interlace_method: u8,
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
          bit_depth: *bit_depth,
          color_type: *color_type,
          interlace_method: *interlace_method,
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

pub fn png_get_header(bytes: &[u8]) -> Option<IHDR> {
  PngRawChunkIter::new(bytes)
    .filter_map(|raw_chunk| {
      let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
      IHDR::try_from(png_chunk).ok()
    })
    .next()
}

pub fn png_get_palette(bytes: &[u8]) -> &[[u8; 3]] {
  PngRawChunkIter::new(bytes)
    .filter_map(|raw_chunk| {
      let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
      let plte = PLTE::try_from(png_chunk).ok()?;
      Some(plte.entries())
    })
    .next()
    .unwrap_or(&[])
}

pub fn png_get_idat(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
  PngRawChunkIter::new(bytes).filter_map(|raw_chunk| {
    let png_chunk = PngChunk::try_from(raw_chunk).ok()?;
    let idat = IDAT::try_from(png_chunk).ok()?;
    Some(idat.as_bytes())
  })
}
