use core::fmt::{Debug, Write};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RawPngChunkTag([u8; 4]);
impl Debug for RawPngChunkTag {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_char(self.0[0] as char)?;
    f.write_char(self.0[1] as char)?;
    f.write_char(self.0[2] as char)?;
    f.write_char(self.0[3] as char)?;
    Ok(())
  }
}
