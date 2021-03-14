use super::*;

#[derive(Debug, Copy, Clone)]
pub struct PngChunk<'b> {
  pub(crate) length: u32,
  pub(crate) chunk_type: ChunkType,
  pub(crate) chunk_data: &'b [u8],
  pub(crate) declared_crc: u32,
}
impl<'b> PngChunk<'b> {
  pub fn get_actual_crc(&self) -> u32 {
    const fn make_crc_table() -> [u32; 256] {
      let mut n = 0_usize;
      let mut table = [0_u32; 256];
      while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
          c = if c & 1 != 0 { 0xedb88320 ^ (c >> 1) } else { c >> 1 };
          //
          k += 1;
        }
        table[n] = c;
        //
        n += 1;
      }
      table
    }
    const CRC_TABLE: [u32; 256] = make_crc_table();
    fn update_crc(mut crc: u32, byte_iter: impl Iterator<Item = u8>) -> u32 {
      for b in byte_iter {
        crc = CRC_TABLE[(crc ^ b as u32) as usize & 0xFF] ^ (crc >> 8);
      }
      crc
    }
    fn crc(byte_iter: impl Iterator<Item = u8>) -> u32 {
      update_crc(u32::MAX, byte_iter) ^ u32::MAX
    }
    crc(self.chunk_type.0.iter().copied().chain(self.chunk_data.iter().copied()))
  }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct ChunkType(pub(crate) [u8; 4]);
#[allow(dead_code)]
impl ChunkType {
  pub const IHDR: Self = ChunkType(*b"IHDR");
  pub const IDAT: Self = ChunkType(*b"IDAT");

  pub const fn is_critical(self) -> bool {
    (self.0[0] & 32) > 0
  }
  pub const fn is_public(self) -> bool {
    (self.0[1] & 32) > 0
  }
  pub const fn is_not_reserved(self) -> bool {
    (self.0[2] & 32) > 0
  }
  pub const fn is_trouble_to_copy(self) -> bool {
    (self.0[3] & 32) > 0
  }
}
impl core::fmt::Debug for ChunkType {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let [a, b, c, d] = self.0;
    write!(f, "{}{}{}{}", a as char, b as char, c as char, d as char)
  }
}
