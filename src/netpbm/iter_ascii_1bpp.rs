use super::{netpbm_trim_comments_and_whitespace, NetpbmError};

/// Parses 1bpp ascii entries.
///
/// For the purposes of this parse whitespace and comments are skipped over as
/// usual, but also whitespace is not even required between entries.
/// * Each `b'0'` outputs as `Ok(false)`
/// * Each `b'1'` outputs as `Ok(true)`
/// * Any other un-skipped character in the output stream gives an error.
pub struct NetpbmAscii1bppIter<'b> {
  spare: &'b [u8],
}
impl<'b> NetpbmAscii1bppIter<'b> {
  pub fn new(bytes: &'b [u8]) -> Self {
    Self { spare: netpbm_trim_comments_and_whitespace(bytes) }
  }
}
impl<'b> core::iter::Iterator for NetpbmAscii1bppIter<'b> {
  type Item = Result<bool, NetpbmError>;
  fn next(&mut self) -> Option<Self::Item> {
    let b = self.spare.get(0)?;
    let out = Some(match b {
      b'0' => Ok(false),
      b'1' => Ok(true),
      _ => Err(NetpbmError::CouldNotParseUnsigned),
    });
    self.spare = netpbm_trim_comments_and_whitespace(&self.spare[1..]);
    out
  }
}
