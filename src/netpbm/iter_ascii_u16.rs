use super::{netpbm_read_ascii_unsigned, netpbm_trim_comments_and_whitespace, NetpbmError};

/// Parses u8 ascii entries.
pub struct NetpbmAsciiU16Iter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAsciiU16Iter<'b> {
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAsciiU16Iter<'b> {
  type Item = Result<u16, NetpbmError>;
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      return None;
    } else {
      match netpbm_read_ascii_unsigned(self.spare) {
        Ok((u, rest)) => {
          self.spare = netpbm_trim_comments_and_whitespace(rest);
          if u <= (u16::MAX as u32) {
            Some(Ok(u as u16))
          } else {
            Some(Err(NetpbmError::IntegerExceedsMaxValue))
          }
        }
        Err(e) => Some(Err(e)),
      }
    }
  }
}
