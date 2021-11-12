#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PngChunkTy([u8; 4]);
impl PngChunkTy {
  pub const IHDR: Self = Self(*b"IHDR");
  pub const IDAT: Self = Self(*b"IDAT");
}
impl core::fmt::Debug for PngChunkTy {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Debug::fmt(core::str::from_utf8(self.0.as_slice()).unwrap_or("?"), f)
  }
}

#[derive(Debug, Clone, Copy)]
pub struct PngChunk<'b> {
  ty: PngChunkTy,
  data: &'b [u8],
  declared_crc: u32,
}
impl<'b> PngChunk<'b> {
  #[inline]
  #[must_use]
  pub const fn ty(&self) -> PngChunkTy {
    self.ty
  }
  #[inline]
  #[must_use]
  pub const fn data(&self) -> &'b [u8] {
    self.data
  }
  #[inline]
  #[must_use]
  pub const fn delcared_crc(&self) -> u32 {
    self.declared_crc
  }
  #[inline]
  #[must_use]
  pub fn compute_actual_crc(&self) -> u32 {
    let mut c = u32::MAX;
    self.ty.0.iter().copied().chain(self.data.iter().copied()).for_each(|b| {
      c = CRC_TABLE[((c ^ (b as u32)) & 0xFF) as usize] ^ (c >> 8);
    });
    c ^ u32::MAX
  }
}

pub struct PngChunkIter<'b> {
  spare: &'b [u8],
}
impl<'b> From<&'b [u8]> for PngChunkIter<'b> {
  #[inline]
  #[must_use]
  fn from(spare: &'b [u8]) -> Self {
    Self { spare }
  }
}
impl<'b> Iterator for PngChunkIter<'b> {
  type Item = PngChunk<'b>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      return None;
    }
    let (len, rest) = if self.spare.len() < 4 {
      self.spare = &[];
      return None;
    } else {
      let (len_bytes, rest) = self.spare.split_at(4);
      let len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
      (len, rest)
    };
    let (ty, rest) = if rest.len() < 4 {
      self.spare = &[];
      return None;
    } else {
      let (ty_bytes, rest) = rest.split_at(4);
      (PngChunkTy(ty_bytes.try_into().unwrap()), rest)
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
    Some(PngChunk { ty, data, declared_crc })
  }
}

const CRC_TABLE: [u32; 256] = {
  let mut table = [0_u32; 256];
  let mut n = 0;
  while n < 256 {
    let mut c: u32 = n as _;
    let mut k = 0;
    while k < 8 {
      if (c & 1) != 0 {
        c = 0xedb88320 ^ (c >> 1);
      } else {
        c = c >> 1;
      }
      //
      k += 1;
    }
    table[n] = c;
    //
    n += 1;
  }
  table
};
