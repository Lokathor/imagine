use super::*;

pub struct BitSource<'b, I> {
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
    if self.current.is_empty() {
      self.current = self.more.next().ok_or(PngError::UnexpectedEndOfInput)?;
    }
    debug_assert!(self.current.len() > 0);
    let (n, c) = self.current.split_at(1);
    self.current = c;
    Ok(n[0])
  }

  fn feed(&mut self, count_after: u32) -> PngResult<()> {
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

  pub fn get_bfinal(&mut self) -> PngResult<bool> {
    if self.spare_bit_count < 1 {
      self.feed(1)?;
    }
    debug_assert!(self.spare_bit_count >= 1);
    let bfinal = (self.spare_bits & 0b1) != 0;
    self.spare_bits >>= 1;
    self.spare_bit_count -= 1;
    Ok(bfinal)
  }

  pub fn get_btype(&mut self) -> PngResult<u32> {
    if self.spare_bit_count < 2 {
      self.feed(2)?;
    }
    debug_assert!(self.spare_bit_count >= 2);
    let btype = self.spare_bits & 0b11;
    self.spare_bits >>= 2;
    self.spare_bit_count -= 2;
    Ok(btype)
  }
}
