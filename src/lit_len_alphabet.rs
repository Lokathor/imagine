use super::*;

#[derive(Clone, Copy)]
pub(crate) struct LitLenAlphabet {
  pub(crate) tree: [TreeEntry; Self::COUNT],
  pub(crate) min_bit_count: u16,
  pub(crate) max_bit_count: u16,
}
impl LitLenAlphabet {
  const COUNT: usize = 286;

  pub fn refresh(&mut self) {
    TreeEntry::fill_in_the_codes(&mut self.tree);

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
  }

  pub fn pull_and_match<'b, I: Iterator<Item = &'b [u8]>>(
    &self, bi: &mut BitSource<'b, I>,
  ) -> PngResult<usize> {
    let mut key = TreeEntry {
      bit_pattern: bi.next_bits_msb(u32::from(self.min_bit_count))? as u16,
      bit_count: self.min_bit_count,
    };
    loop {
      if let Some(pos) = self.tree.iter().position(|&te| te == key) {
        return Ok(pos);
      } else {
        // new bits are pushed onto the bottom of our temporary key, like how
        // `read_bits_most` works.
        key.bit_pattern <<= 1;
        key.bit_pattern |= bi.next_one_bit()? as u16;
        key.bit_count += 1;
        if key.bit_count > self.max_bit_count {
          return Err(PngError::CouldNotFindLitLenSymbol);
        }
      }
    }
  }
}
impl core::fmt::Debug for LitLenAlphabet {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    write!(f, "LitLenAlphabet {{ tree: {:?} }}", &self.tree[..])
  }
}
impl core::cmp::PartialEq for LitLenAlphabet {
  fn eq(&self, other: &Self) -> bool {
    self.tree[..] == other.tree[..]
  }
}
impl core::cmp::Eq for LitLenAlphabet {}
impl Default for LitLenAlphabet {
  fn default() -> Self {
    Self { tree: [TreeEntry::default(); Self::COUNT], min_bit_count: 0, max_bit_count: 0 }
  }
}
