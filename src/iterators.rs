/// Iterates every 1 bit of the byte, going from high to low.
///
/// This returns all bits in the sequence, so use `take` as necessary.
#[inline]
#[must_use]
pub fn iter_1bpp_high_to_low(bytes: &[u8]) -> impl Iterator<Item = bool> + '_ {
  bytes
    .iter()
    .copied()
    .map(|byte| {
      [
        (byte & 0b1000_0000) != 0,
        (byte & 0b0100_0000) != 0,
        (byte & 0b0010_0000) != 0,
        (byte & 0b0001_0000) != 0,
        (byte & 0b0000_1000) != 0,
        (byte & 0b0000_0100) != 0,
        (byte & 0b0000_0010) != 0,
        (byte & 0b0000_0001) != 0,
      ]
      .into_iter()
    })
    .flatten()
}

/// Iterates every 2 bits of the byte, going from high to low.
///
/// This returns all bits in the sequence, so use `take` as necessary.
#[inline]
#[must_use]
pub fn iter_2bpp_high_to_low(bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
  bytes
    .iter()
    .copied()
    .map(|byte| {
      [
        (byte & 0b1100_0000) >> 6,
        (byte & 0b0011_0000) >> 4,
        (byte & 0b0000_1100) >> 2,
        (byte & 0b0000_0011) >> 0,
      ]
      .into_iter()
    })
    .flatten()
}

/// Iterates every 4 bits of the byte, going from high to low.
///
/// This returns all bits in the sequence, so use `take` as necessary.
#[inline]
#[must_use]
pub fn iter_4bpp_high_to_low(bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
  bytes
    .iter()
    .copied()
    .map(|byte| [(byte & 0b1111_0000) >> 4, (byte & 0b0000_1111) >> 0].into_iter())
    .flatten()
}

/// Takes an iterator and gathers up `N` elements at a time into an array.
///
/// This is the reverse of "flatten", thus it is a "bulken".
///
/// Note: The inner iterator's items must be [Copy] because if the inner
/// iterator runs out while this iterator is trying to build up an array of
/// output then all the intermediate values will be discarded. Technically it's
/// safe to leak values, but to avoid accidental leaks, the `Copy` bound is
/// placed. If you do want a leaky version, just copy and paste this somewhere
/// with a new name and take off the `Copy` bound.
pub struct BulkenIter<I, const N: usize>(pub I)
where
  I: Iterator,
  I::Item: Copy;
impl<I, const N: usize> Iterator for BulkenIter<I, N>
where
  I: Iterator,
  I::Item: Copy,
{
  type Item = [I::Item; N];

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    use core::mem::MaybeUninit;

    // Safety: This unwraps a `MaybeUninit<[MaybeUninit<I::Item>; N]>` into the
    // inner `[MaybeUninit<I::Item>; N]` array, which is always safe because all
    // bytes of the array are still tagged as `MaybeUninit` values.
    let mut a: [MaybeUninit<I::Item>; N] = unsafe { MaybeUninit::uninit().assume_init() };
    // TODO: When MaybeUninit::uninit_array is stabilized, use that.
    for a_mut in a.iter_mut() {
      *a_mut = MaybeUninit::new(self.0.next()?);
    }
    // Safety: This reads off the pointer as the initialized inner type, which
    // is sound because we initialized all elements and MaybeUninit is
    // repr(transparent).
    Some(unsafe { core::ptr::read(&a as *const [MaybeUninit<I::Item>; N] as *const [I::Item; N]) })
    // TODO: When MaybeUninit::array_assume_init is stabilized, use that.
  }
}
