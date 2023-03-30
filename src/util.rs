#![allow(dead_code)]

use bytemuck::{checked::pod_read_unaligned, Pod};

use crate::ImagineError;
use core::{
  mem::size_of,
  num::{NonZeroU16, NonZeroU32},
};

#[inline]
pub(crate) fn try_pull_byte_array<const N: usize>(
  bytes: &[u8],
) -> Result<([u8; N], &[u8]), ImagineError> {
  if bytes.len() >= N {
    let (head, tail) = bytes.split_at(N);
    let a: [u8; N] = head.try_into().unwrap();
    Ok((a, tail))
  } else {
    Err(ImagineError::Parse)
  }
}

#[inline]
pub(crate) fn try_pull_pod<T: Pod>(bytes: &[u8]) -> Result<(T, &[u8]), ImagineError> {
  let position = size_of::<T>();
  if bytes.len() >= position {
    let (head, tail) = bytes.split_at(position);
    let a: T = pod_read_unaligned(head);
    Ok((a, tail))
  } else {
    Err(ImagineError::Parse)
  }
}

#[inline]
#[must_use]
pub(crate) fn u16_le(bytes: &[u8]) -> u16 {
  u16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub(crate) fn i16_le(bytes: &[u8]) -> i16 {
  i16::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub(crate) fn u32_le(bytes: &[u8]) -> u32 {
  u32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub(crate) fn i32_le(bytes: &[u8]) -> i32 {
  i32::from_le_bytes(bytes.try_into().unwrap())
}

#[inline]
#[must_use]
pub(crate) fn onz_u16_le(bytes: &[u8]) -> Option<NonZeroU16> {
  NonZeroU16::new(u16_le(bytes))
}

#[inline]
#[must_use]
pub(crate) fn onz_u32_le(bytes: &[u8]) -> Option<NonZeroU32> {
  NonZeroU32::new(u32_le(bytes))
}
