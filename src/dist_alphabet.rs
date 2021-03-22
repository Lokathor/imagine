use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DistAlphabet {
  pub(crate) tree: [TreeEntry; Self::COUNT],
  pub(crate) min_bit_count: u16,
  pub(crate) max_bit_count: u16,
}
impl DistAlphabet {
  const COUNT: usize = 30;

  pub fn refresh(&mut self) -> PngResult<()> {
    TreeEntry::fill_in_the_codes(&mut self.tree)?;

    self.min_bit_count = 15;
    self.max_bit_count = 0;
    for te in self.tree.iter().copied() {
      if te.bit_count == 0 {
        continue;
      }
      self.min_bit_count = self.min_bit_count.min(te.bit_count);
      self.max_bit_count = self.max_bit_count.max(te.bit_count);
    }
    debug_assert!(
      self.min_bit_count <= self.max_bit_count,
      "min {}, max {}",
      self.min_bit_count,
      self.max_bit_count
    );
    Ok(())
  }

  pub fn pull_and_match<'b, I: Iterator<Item = &'b [u8]>>(
    &self, bi: &mut BitSource<'b, I>,
  ) -> PngResult<usize> {
    //trace!("DistAlphabet::pull_and_match");
    //dump!(self.min_bit_count);
    let mut key = TreeEntry {
      bit_pattern: bi.next_bits_msb(u32::from(self.min_bit_count))? as u16,
      bit_count: self.min_bit_count,
    };
    loop {
      //dump!(key);
      if let Some(pos) = self.tree.iter().position(|&te| te == key) {
        //trace!("matched at position {}", pos);
        return Ok(pos);
      } else {
        // new bits are pushed onto the bottom of our temporary key, like how
        // `read_bits_most` works.
        key.bit_pattern <<= 1;
        key.bit_pattern |= bi.next_one_bit()? as u16;
        key.bit_count += 1;
        if key.bit_count > self.max_bit_count {
          return Err(PngError::CouldNotFindDistSymbol);
        }
      }
    }
  }
}
