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
    if self.bytes.len() < 12 {
      return None;
    }
    let length = u32::from_be_bytes(self.bytes[0..4].try_into().unwrap());
    let chunk_type = ChunkType(self.bytes[4..8].try_into().unwrap());
    if self.bytes.len() < (length as usize) + 4 {
      return None;
    }
    let chunk_data = &self.bytes[8..(8 + length as usize)];
    let declared_crc = u32::from_be_bytes(
      self.bytes[(8 + length as usize)..(8 + length as usize + 4)].try_into().unwrap(),
    );
    self.bytes = &self.bytes[(8 + length as usize + 4)..];
    Some(PngChunk { length, chunk_type, chunk_data, declared_crc })
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let more_chunks = self.bytes.len() >= 12;
    (more_chunks as usize, Some(self.bytes.len() / 12))
  }
}
