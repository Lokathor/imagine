/// A `u16` stored as big-endian bytes.
///
/// This stores only an array of bytes, so unlike a normal `u16` it has an
/// alignment of 1.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct U16BE([u8; 2]);
impl U16BE {
  /// Convert this value to a native `u16`
  #[inline]
  #[must_use]
  pub const fn to_u16(self) -> u16 {
    u16::from_be_bytes(self.0)
  }
  /// Make a value from a native `u16`
  #[inline]
  #[must_use]
  pub const fn from_u16(u: u16) -> Self {
    Self(u.to_be_bytes())
  }
}
impl core::fmt::Debug for U16BE {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("U16BE").field(&self.to_u16()).finish()
  }
}
impl From<u16> for U16BE {
  #[inline]
  #[must_use]
  fn from(value: u16) -> Self {
    Self::from_u16(value)
  }
}
impl From<U16BE> for u16 {
  #[inline]
  #[must_use]
  fn from(value: U16BE) -> Self {
    value.to_u16()
  }
}

/// A `u32` stored as big-endian bytes.
///
/// This stores only an array of bytes, so unlike a normal `u32` it has an
/// alignment of 1.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct U32BE([u8; 4]);
impl U32BE {
  /// Convert this value to a native `u32`
  #[inline]
  #[must_use]
  pub const fn to_u32(self) -> u32 {
    u32::from_be_bytes(self.0)
  }
  /// Make a value from a native `u32`
  #[inline]
  #[must_use]
  pub const fn from_u32(u: u32) -> Self {
    Self(u.to_be_bytes())
  }
}
impl core::fmt::Debug for U32BE {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("U32BE").field(&self.to_u32()).finish()
  }
}
impl From<u32> for U32BE {
  #[inline]
  #[must_use]
  fn from(value: u32) -> Self {
    Self::from_u32(value)
  }
}
impl From<U32BE> for u32 {
  #[inline]
  #[must_use]
  fn from(value: U32BE) -> Self {
    value.to_u32()
  }
}
