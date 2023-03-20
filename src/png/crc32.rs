const CRC_TABLE: [u32; 256] = make_crc_table();

const fn make_crc_table() -> [u32; 256] {
  let mut out = [0; 256];
  let mut n = 0;
  while n < 256 {
    let mut c = n as u32;
    let mut k = 0;
    while k < 8 {
      if (c & 1) != 0 {
        c = 0xEDB8_8320_u32 ^ (c >> 1);
      } else {
        c = c >> 1;
      }
      //
      k += 1;
    }
    out[n] = c;
    //
    n += 1;
  }
  out
}

fn update_crc(mut crc: u32, iter: impl Iterator<Item = u8>) -> u32 {
  for byte in iter {
    let i = (crc ^ u32::from(byte)) as u8 as usize;
    crc = CRC_TABLE[i] ^ (crc >> 8);
  }
  crc
}

#[inline]
pub(crate) fn png_crc(iter: impl Iterator<Item = u8>) -> u32 {
  update_crc(u32::MAX, iter) ^ u32::MAX
}
