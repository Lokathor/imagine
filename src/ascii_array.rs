use core::fmt::Write;

/// An array of bytes expected to contain ascii data.
///
/// There's no actual enforced encoding! The `Debug` and `Display` impls will
/// just `as` cast each byte into a character. This works just as expected for
/// ascii data (`32..=126`), and is still safe for non-ascii data, but you just
/// might get non-printing characters or multi-byte unicode characters.
///
/// This type really just exists to provide alternate `Debug` and `Display`
/// impls for byte arrays. Image formats have magic byte sequences
/// which are intended to match ascii sequences (such as PNG header tags), and
/// so this is a useful newtype to use in other structures to give them a better
/// `Debug` output.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AsciiArray<const N: usize>(pub [u8; N]);

impl<const N: usize> Default for AsciiArray<N> {
  #[inline]
  #[must_use]
  fn default() -> Self {
    unsafe { core::mem::zeroed() }
  }
}

impl<const N: usize> core::fmt::Debug for AsciiArray<N> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_char('\"')?;
    for ch in self.0.iter().copied().map(|u| u as char) {
      f.write_char(ch)?;
    }
    f.write_char('\"')?;
    Ok(())
  }
}
impl<const N: usize> core::fmt::Display for AsciiArray<N> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    for ch in self.0.iter().copied().map(|u| u as char) {
      f.write_char(ch)?;
    }
    Ok(())
  }
}

impl<const N: usize> From<[u8; N]> for AsciiArray<N> {
  #[inline]
  #[must_use]
  fn from(array: [u8; N]) -> Self {
    Self(array)
  }
}
