use super::*;

pub struct PngChunkIter<'b> {
  bytes: &'b [u8],
}
impl<'b> PngChunkIter<'b> {
  pub fn from_png_bytes(bytes: &'b [u8]) -> PngResult<Self> {
    fn drop_png_signature(bytes: &[u8]) -> PngResult<&[u8]> {
      const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
      if bytes.len() < 8 {
        Err(PngError::UnexpectedEndOfInput)
      } else if &bytes[..8] == &PNG_SIGNATURE {
        Ok(&bytes[8..])
      } else {
        Err(PngError::NoPngSignature)
      }
    }
    drop_png_signature(bytes).map(|bytes| Self { bytes })
  }
}
impl<'b> Iterator for PngChunkIter<'b> {
  type Item = PngChunk<'b>;

  fn next(&mut self) -> Option<PngChunk<'b>> {
    // Note(Lokathor): this combines the bounds check of the length and
    // chunk_type since they're both a fixed size.
    if self.bytes.len() < 8 {
      self.bytes = &[];
      return None;
    }
    //
    let length = {
      let (l_bytes, rest) = self.bytes.split_at(4);
      self.bytes = rest;
      u32::from_be_bytes(l_bytes.try_into().unwrap())
    };
    //
    let chunk_type = {
      let (chunk_bytes, rest) = self.bytes.split_at(4);
      self.bytes = rest;
      ChunkType(chunk_bytes.try_into().unwrap())
    };
    // Note(Lokathor): Now we combine the bounds checks for both the data and
    // the CRC value after the data.
    if self.bytes.len() < (length as usize) + 4 {
      self.bytes = &[];
      return None;
    };
    let chunk_data = {
      let (chunk_bytes, rest) = self.bytes.split_at(length as usize);
      self.bytes = rest;
      chunk_bytes
    };
    let declared_crc = {
      let (declared_crc_bytes, rest) = self.bytes.split_at(4);
      self.bytes = rest;
      u32::from_be_bytes(declared_crc_bytes.try_into().unwrap())
    };
    Some(PngChunk { length, chunk_type, chunk_data, declared_crc })
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let more_chunks = self.bytes.len() >= 12;
    (more_chunks as usize, Some(self.bytes.len() / 12))
  }
}
