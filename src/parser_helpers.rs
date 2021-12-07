#![forbid(unsafe_code)]

//! Just has shorthands for various int parsing things you'd want to do.

use core::num::{NonZeroU16, NonZeroU32};

#[inline]
#[must_use]
pub fn u16_le(bytes: &[u8]) -> u16 {
  u16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub fn i16_le(bytes: &[u8]) -> i16 {
  i16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub fn u32_le(bytes: &[u8]) -> u32 {
  u32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub fn i32_le(bytes: &[u8]) -> i32 {
  i32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub fn onz_u16_le(bytes: &[u8]) -> Option<NonZeroU16> {
  NonZeroU16::new(u16_le(bytes))
}

#[inline]
#[must_use]
pub fn onz_u32_le(bytes: &[u8]) -> Option<NonZeroU32> {
  NonZeroU32::new(u32_le(bytes))
}

pub fn try_split_off_byte_array<const N: usize>(bytes: &[u8]) -> Option<([u8; N], &[u8])> {
  if bytes.len() >= N {
    let (head, tail) = bytes.split_at(N);
    let a: [u8; N] = head.try_into().unwrap();
    Some((a, tail))
  } else {
    None
  }
}
