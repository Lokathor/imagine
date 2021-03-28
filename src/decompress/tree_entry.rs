use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TreeEntry {
  pub(crate) bit_pattern: u16,
  pub(crate) bit_count: u16,
}
impl core::fmt::Debug for TreeEntry {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    if f.alternate() {
      write!(
        f,
        "TreeEntry {{ bit_pattern: {}, bit_count: {}, }}",
        self.bit_pattern, self.bit_count
      )
    } else {
      let temp = format!("{:016b}", self.bit_pattern);
      write!(f, "TE {{ \"{}\" }}", &temp[(16 - self.bit_count) as usize..])
    }
  }
}

impl TreeEntry {
  /// Given a list of filled in `bit_counts`, computes the `bit_patterns`.
  ///
  /// * `bit_count` must be 15 or less or it will be ignored.
  /// * `bit_count` 0 means that the TreeEntry doesn't participate in
  ///   `bit_pattern` generation at all.
  pub(crate) fn fill_in_the_codes(tree: &mut [TreeEntry]) -> PngResult<()> {
    assert!(
      tree.len() > 0,
      "It's a programmer error if you're trying to fill in an empty tree"
    );

    let max_bits = usize::from(tree.iter().map(|te| te.bit_count).max().unwrap());

    // 1) Count the number of codes for each code length.
    let mut bl_count = [0_u16; 16];
    for (count, blc) in bl_count.iter_mut().enumerate() {
      let ct = count as u16;
      *blc = tree.iter().filter(|te| te.bit_count == ct).count() as u16;
    }

    // 2) Find the numerical value of the smallest code for each code length
    let mut next_code = [0_u16; 16];
    let mut code = 0_u16;
    bl_count[0] = 0;
    for bits in 1..=max_bits {
      code = (code + bl_count[bits - 1]) << 1;
      //trace!("smallest code for {} bits is {:016b}", bits, code);
      next_code[bits] = code;
    }

    // 3) Assign numerical values to all codes, using consecutive values for all
    //    codes of the same length with the base values determined at step 2.
    //    Codes that are never used (which have a bit length of zero) must not be
    //    assigned a value.
    for te in tree.iter_mut() {
      let len = usize::from(te.bit_count);
      if len != 0 {
        if next_code[len] & !((1 << len) - 1) == 0 {
          te.bit_pattern = next_code[len];
          next_code[len] += 1;
        } else {
          return Err(PngError::BadDynamicHuffmanTreeData);
        }
      }
    }

    Ok(())
  }
}

#[test]
fn test_fill_in_the_codes() {
  // the small example in the spec.
  let mut test_tree = [
    TreeEntry { bit_count: 2, bit_pattern: 0 },
    TreeEntry { bit_count: 1, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
  ];
  TreeEntry::fill_in_the_codes(&mut test_tree);
  let expected_tree = [
    TreeEntry { bit_count: 2, bit_pattern: 0b10 },
    TreeEntry { bit_count: 1, bit_pattern: 0b0 },
    TreeEntry { bit_count: 3, bit_pattern: 0b110 },
    TreeEntry { bit_count: 3, bit_pattern: 0b111 },
  ];
  assert_eq!(test_tree, expected_tree);

  trace!("===");

  // the bigger example in the spec.
  let mut test_tree = [
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 3, bit_pattern: 0 },
    TreeEntry { bit_count: 2, bit_pattern: 0 },
    TreeEntry { bit_count: 4, bit_pattern: 0 },
    TreeEntry { bit_count: 4, bit_pattern: 0 },
  ];
  TreeEntry::fill_in_the_codes(&mut test_tree);
  let expected_tree = [
    TreeEntry { bit_count: 3, bit_pattern: 0b010 },
    TreeEntry { bit_count: 3, bit_pattern: 0b011 },
    TreeEntry { bit_count: 3, bit_pattern: 0b100 },
    TreeEntry { bit_count: 3, bit_pattern: 0b101 },
    TreeEntry { bit_count: 3, bit_pattern: 0b110 },
    TreeEntry { bit_count: 2, bit_pattern: 0b00 },
    TreeEntry { bit_count: 4, bit_pattern: 0b1110 },
    TreeEntry { bit_count: 4, bit_pattern: 0b1111 },
  ];
  assert_eq!(test_tree, expected_tree);

  trace!("===");

  /* Table from the "compressed with fixed huffman codes"

      Lit Value     Bits    Codes
      ---------     ----    ----
      0 - 143       8       00110000 through
                            10111111
      144 - 255     9       110010000 through
                            111111111
      256 - 279     7       0000000 through
                            0010111
      280 - 287     8       11000000 through
                            11000111
  */
  let mut v = Vec::with_capacity(288);
  for _ in 0..=143 {
    v.push(TreeEntry { bit_count: 8, bit_pattern: 0 });
  }
  for _ in 144..=255 {
    v.push(TreeEntry { bit_count: 9, bit_pattern: 0 });
  }
  for _ in 256..=279 {
    v.push(TreeEntry { bit_count: 7, bit_pattern: 0 });
  }
  for _ in 280..=287 {
    v.push(TreeEntry { bit_count: 8, bit_pattern: 0 });
  }
  TreeEntry::fill_in_the_codes(&mut v);
  //
  assert_eq!(v[0].bit_pattern, 0b00110000);
  assert_eq!(v[143].bit_pattern, 0b10111111);
  //
  assert_eq!(v[144].bit_pattern, 0b110010000);
  assert_eq!(v[255].bit_pattern, 0b111111111);
  //
  assert_eq!(v[256].bit_pattern, 0b0000000);
  assert_eq!(v[279].bit_pattern, 0b0010111);
  //
  assert_eq!(v[280].bit_pattern, 0b11000000);
  assert_eq!(v[287].bit_pattern, 0b11000111);
  //
}
