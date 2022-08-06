#![allow(bad_style)]

mod png;

fn rand_bytes(count: usize) -> Vec<u8> {
  let mut buffer = vec![0; count];
  getrandom::getrandom(&mut buffer).unwrap();
  buffer
}
