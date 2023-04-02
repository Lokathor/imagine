use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct PngRawChunkType(pub(crate) [u8; 4]);
#[allow(nonstandard_style)]
impl PngRawChunkType {
  pub const IHDR: Self = Self(*b"IHDR");
  pub const PLTE: Self = Self(*b"PLTE");
  pub const IDAT: Self = Self(*b"IDAT");
  pub const IEND: Self = Self(*b"IEND");
  pub const tRNS: Self = Self(*b"tRNS");
  pub const bKGD: Self = Self(*b"bKGD");
  pub const sRGB: Self = Self(*b"sRGB");
  pub const gAMA: Self = Self(*b"gAMA");
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
  pub(crate) type_: PngRawChunkType,
  pub(crate) data: &'b [u8],
  pub(crate) declared_crc: u32,
}
impl Debug for PngRawChunk<'_> {
  #[inline]
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
pub struct PngRawChunkIter<'b>(pub(crate) &'b [u8]);
impl<'b> PngRawChunkIter<'b> {
  /// Pass the full PNG bytes, it will remove the PNG header automatically.
  #[inline]
  pub const fn new(bytes: &'b [u8]) -> Self {
    match bytes {
      [_, _, _, _, _, _, _, _, rest @ ..] => Self(rest),
      _ => Self(&[]),
    }
  }
}
impl<'b> Iterator for PngRawChunkIter<'b> {
  type Item = PngRawChunk<'b>;
  #[inline]
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
