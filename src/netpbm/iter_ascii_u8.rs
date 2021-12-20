use super::{netpbm_read_ascii_unsigned, netpbm_trim_comments_and_whitespace, NetpbmError};

/// Parses u8 ascii entries.
pub struct NetpbmAsciiU8Iter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAsciiU8Iter<'b> {
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAsciiU8Iter<'b> {
  type Item = Result<u8, NetpbmError>;
  fn next(&mut self) -> Option<Self::Item> {
    if self.spare.is_empty() {
      return None;
    } else {
      match netpbm_read_ascii_unsigned(self.spare) {
        Ok((u, rest)) => {
          self.spare = netpbm_trim_comments_and_whitespace(rest);
          if u <= (u8::MAX as u32) {
            Some(Ok(u as u8))
          } else {
            Some(Err(NetpbmError::IntegerExceedsMaxValue))
          }
        }
        Err(e) => Some(Err(e)),
      }
    }
  }
}
