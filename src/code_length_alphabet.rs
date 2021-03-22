use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct CodeLengthAlphabet {
  /// important data
  pub tree: [TreeEntry; Self::COUNT],
  /// min and max just help speed up the matching process
  pub min_bit_count: u16,
  pub max_bit_count: u16,
}
impl CodeLengthAlphabet {
  const COUNT: usize = 19;

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

  fn pull_and_match<'b, I: Iterator<Item = &'b [u8]>>(
    &self, bi: &mut BitSource<'b, I>,
  ) -> PngResult<usize> {
    //trace!("CodeLengthAlphabet::pull_and_match");
    //dump!(self.min_bit_count);
    let mut key = TreeEntry {
      bit_pattern: bi.next_bits_msb(u32::from(self.min_bit_count))? as u16,
      bit_count: self.min_bit_count,
    };
    loop {
      //dump!(key);
      if let Some(pos) = self.tree.iter().position(|&te| te == key) {
        //trace!("matched at position {}, returning.", pos);
        return Ok(pos);
      } else {
        // new bits are pushed onto the bottom of our temporary key, like how
        // `read_bits_most` works.
        key.bit_pattern <<= 1;
        key.bit_pattern |= bi.next_one_bit()? as u16;
        key.bit_count += 1;
      }
    }
  }

  pub fn fill_a_tree<'b, I: Iterator<Item = &'b [u8]>>(
    &self, element_count: usize, tree: &mut [TreeEntry], bi: &mut BitSource<'b, I>,
  ) -> PngResult<()> {
    let mut code_lengths_acquired = 0_usize;
    while code_lengths_acquired < element_count {
      let cl_match = self.pull_and_match(bi)? as u16;
      //dump!(cl_match);
      match cl_match {
        0..=15 => {
          //trace!("literal {}", cl_match);
          tree[code_lengths_acquired].bit_count = cl_match;
          code_lengths_acquired += 1;
        }
        16 => {
          //trace!("previous");
          if code_lengths_acquired == 0 {
            return Err(PngError::BadDynamicHuffmanTreeData);
          }
          let bits_of_length = bi.next_bits_lsb(2)?;
          let repeat_count = 3 + bits_of_length;
          debug_assert!(repeat_count >= 3 && repeat_count <= 6);
          for _ in 0..repeat_count {
            tree[code_lengths_acquired].bit_count =
              tree[code_lengths_acquired - 1].bit_count;
            code_lengths_acquired += 1;
          }
        }
        17 => {
          //trace!("short 0 sequence");
          let bits_of_length = bi.next_bits_lsb(3)?;
          let repeat_count = 3 + bits_of_length;
          //dump!(bits_of_length, repeat_count);
          debug_assert!(repeat_count >= 3 && repeat_count <= 10);
          for _ in 0..repeat_count {
            tree[code_lengths_acquired].bit_count = 0;
            code_lengths_acquired += 1;
          }
        }
        18 => {
          //trace!("long 0 sequence");
          let bits_of_length = bi.next_bits_lsb(7)?;
          let repeat_count = 11 + bits_of_length;
          //dump!(bits_of_length, repeat_count);
          debug_assert!(repeat_count >= 11 && repeat_count <= 138);
          for _ in 0..repeat_count {
            tree[code_lengths_acquired].bit_count = 0;
            code_lengths_acquired += 1;
          }
        }
        _ => panic!("illegal match position: {}", cl_match),
      }
      //dump!(code_lengths_acquired);
    }
    Ok(())
  }
}
