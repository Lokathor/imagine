#![allow(bad_style)]
#![allow(unused_imports)]

#[cfg(feature = "alloc")]
use imagine::image::Bitmap;

#[cfg(feature = "png")]
mod png;

#[allow(dead_code)]
fn rand_bytes(count: usize) -> Vec<u8> {
  let mut buffer = vec![0; count];
  getrandom::getrandom(&mut buffer).unwrap();
  buffer
}

#[test]
#[cfg(feature = "alloc")]
fn test_image_vertical_flip() {
  let mut i = Bitmap { width: 3, height: 3, pixels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9] };
  i.vertical_flip();
  assert_eq!(i.pixels, vec![7, 8, 9, 4, 5, 6, 1, 2, 3]);
  //
  let mut i = Bitmap { width: 4, height: 2, pixels: vec![1, 2, 3, 4, 5, 6, 7, 8] };
  i.vertical_flip();
  assert_eq!(i.pixels, vec![5, 6, 7, 8, 1, 2, 3, 4]);
}
