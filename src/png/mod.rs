use core::fmt::{Debug, Write};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RawPngChunkType([u8; 4]);
impl Debug for RawPngChunkType {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_char(self.0[0] as char)?;
    f.write_char(self.0[1] as char)?;
    f.write_char(self.0[2] as char)?;
    f.write_char(self.0[3] as char)?;
    Ok(())
  }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawPngChunk<'b> {
  type_: RawPngChunkType,
  data: &'b [u8],
  declared_crc: u32,
}
impl Debug for RawPngChunk<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("RawPngChunk")
      .field("type_", &self.type_)
      .field("data", &(&self.data[..self.data.len().min(20)], self.data.len()))
      .field("declared_crc", &self.declared_crc)
      .finish()
  }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RawPngChunkIter<'b>(&'b [u8]);
impl<'b> RawPngChunkIter<'b> {
  pub const fn new(bytes: &'b [u8]) -> Self {
    Self(bytes)
  }
}
impl<'b> Iterator for RawPngChunkIter<'b> {
  type Item = RawPngChunk<'b>;
  fn next(&mut self) -> Option<Self::Item> {
    let chunk_len: u32 = if self.0.len() >= 4 {
      let (len_bytes, rest) = self.0.split_at(4);
      self.0 = rest;
      u32::from_be_bytes(len_bytes.try_into().unwrap())
    } else {
      return None;
    };
    let type_: RawPngChunkType = if self.0.len() >= 4 {
      let (type_bytes, rest) = self.0.split_at(4);
      self.0 = rest;
      RawPngChunkType(type_bytes.try_into().unwrap())
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
    Some(RawPngChunk { type_, data, declared_crc })
  }
}
