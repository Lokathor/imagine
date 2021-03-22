use super::*;

pub(crate) struct BitSource<'b, I> {
  current: &'b [u8],
  more: I,
  spare_bits: u32,
  spare_bit_count: u32,
}

impl<'b, I: Iterator> core::fmt::Debug for BitSource<'b, I> {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    let current = {
      match &self.current {
        [] => format!("||"),
        [a] => format!("|{a:08b}|", a = a),
        [a, b] => format!("|{b:08b}|{a:08b}|", a = a, b = b),
        [a, b, ..] => format!("..|{b:08b}|{a:08b}|", a = a, b = b),
      }
    };
    let more_size_hint = self.more.size_hint();
    let spare_bit_count = self.spare_bit_count as usize;
    let spare_bits = if spare_bit_count > 0 {
      format!("{bits:0width$b}", bits = self.spare_bits, width = spare_bit_count)
    } else {
      format!("")
    };
    f.debug_struct("BitSource")
      .field("current", &current)
      .field("spare_bits", &spare_bits)
      .field("spare_bit_count", &spare_bit_count)
      .field("more", &"?")
      .finish()
  }
}

impl<'b, I> BitSource<'b, I> {
  pub const fn new(current: &'b [u8], more: I) -> Self {
    Self { current, more, spare_bits: 0, spare_bit_count: 0 }
  }
}

impl<'b, I: Iterator<Item = &'b [u8]>> BitSource<'b, I> {
  fn grab_byte(&mut self) -> PngResult<u8> {
    trace!("grab_byte");
    if self.current.is_empty() {
      trace!("current slice empty, pulling from iterator");
      self.current = self.more.next().ok_or(PngError::UnexpectedEndOfInput)?;
    }
    debug_assert!(self.current.len() > 0);
    let (n, c) = self.current.split_at(1);
    self.current = c;
    Ok(n[0])
  }

  fn feed(&mut self, count_after: u32) -> PngResult<()> {
    trace!("feed({})", count_after);
    debug_assert!(count_after > 0);
    debug_assert!(count_after < 24);
    debug_assert!(count_after > self.spare_bit_count);
    while count_after > self.spare_bit_count {
      let new_byte = self.grab_byte()? as u32;
      self.spare_bits |= new_byte << self.spare_bit_count;
      self.spare_bit_count += 8;
    }
    Ok(())
  }

  pub fn next_bfinal(&mut self) -> PngResult<bool> {
    self.next_one_bit()
  }

  pub fn next_btype(&mut self) -> PngResult<usize> {
    self.next_bits_lsb(2)
  }

  pub fn next_one_bit(&mut self) -> PngResult<bool> {
    trace!("next_one_bit, spare bits: {}", self.spare_bit_count);
    if self.spare_bit_count < 1 {
      self.feed(1)?;
    }
    debug_assert!(self.spare_bit_count >= 1);
    let bfinal = (self.spare_bits & 0b1) != 0;
    self.spare_bits >>= 1;
    self.spare_bit_count -= 1;
    Ok(bfinal)
  }

  /// Read the next `count` bits, use with huffman data elements.
  pub fn next_bits_msb(&mut self, count: u32) -> PngResult<usize> {
    trace!("next_bits_msb({})", count);
    if self.spare_bit_count < count {
      self.feed(count)?;
    }
    debug_assert!(self.spare_bit_count >= count);
    let bits = {
      let rev = (self.spare_bits & ((1 << count) - 1)).reverse_bits();
      rev >> (32 - count)
    };
    self.spare_bits >>= count;
    self.spare_bit_count -= count;
    Ok(bits as usize)
    // old style
    //let mut out = 0;
    //for _ in 0..count {
    //  out <<= 1;
    //  out |= usize::from(self.next_one_bit()?);
    //}
    //Ok(out)
  }

  /// Read the next `count` bits, use with non-huffman data elements.
  pub fn next_bits_lsb(&mut self, count: u32) -> PngResult<usize> {
    trace!("next_bits_lsb({})", count);
    if self.spare_bit_count < count {
      self.feed(count)?;
    }
    debug_assert!(self.spare_bit_count >= count);
    let bits = self.spare_bits & ((1 << count) - 1);
    self.spare_bits >>= count;
    self.spare_bit_count -= count;
    Ok(bits as usize)
    // old style
    //let mut out = 0;
    //for tracker in 0..count {
    //  out |= usize::from(self.next_one_bit()?) << tracker;
    //}
    //Ok(out)
  }

  pub fn flush_spare_bits(&mut self) {
    trace!("flush_spare_bits");
    self.spare_bit_count = 0;
    self.spare_bits = 0;
  }

  pub fn next_len_nlen(&mut self) -> PngResult<(u16, u16)> {
    debug_assert_eq!(0, self.spare_bit_count);
    debug_assert_eq!(0, self.spare_bits);
    let a = self.grab_byte()?;
    let b = self.grab_byte()?;
    let c = self.grab_byte()?;
    let d = self.grab_byte()?;
    Ok((u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d])))
  }

  pub fn next_up_to_n_bytes(&mut self, n: usize) -> PngResult<&[u8]> {
    trace!("next_up_to_n_bytes");
    if self.current.is_empty() {
      trace!("current slice empty, pulling from iterator");
      self.current = self.more.next().ok_or(PngError::UnexpectedEndOfInput)?;
    }
    if self.current.len() > n {
      // grab only part of current
      let (a, b) = self.current.split_at(n);
      self.current = b;
      Ok(a)
    } else {
      // grab *all* of current
      let out = self.current;
      self.current = &[];
      Ok(out)
    }
  }
}
