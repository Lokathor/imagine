#![allow(bad_style)]

#[cfg(feature = "png")]
mod png;

#[allow(dead_code)]
fn rand_bytes(count: usize) -> Vec<u8> {
  let mut buffer = vec![0; count];
  getrandom::getrandom(&mut buffer).unwrap();
  buffer
}
