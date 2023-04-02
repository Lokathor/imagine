#[test]
fn test_reduced_image_dimensions() {
  use super::*;

  assert_eq!(reduced_image_dimensions(0, 0), [(0, 0); 8]);
  // one
  for (w, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(w, 0)[1].0, ex, "failed w:{w}");
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[1].1, ex, "failed h:{h}");
  }
  // two
  for (w, ex) in (1..=8).zip([0, 0, 0, 0, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(w, 0)[2].0, ex, "failed w:{w}");
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[2].1, ex, "failed h:{h}");
  }
  // three
  for (w, ex) in (1..=8).zip([1, 1, 1, 1, 2, 2, 2, 2]) {
    assert_eq!(reduced_image_dimensions(w, 0)[3].0, ex, "failed w: {w}");
  }
  for (h, ex) in (1..=8).zip([0, 0, 0, 0, 1, 1, 1, 1]) {
    assert_eq!(reduced_image_dimensions(0, h)[3].1, ex, "failed h: {h}");
  }
  // four
  for (w, ex) in (1..=8).zip([0, 0, 1, 1, 1, 1, 2, 2]) {
    assert_eq!(reduced_image_dimensions(w, 0)[4].0, ex, "failed w: {w}");
  }
  for (h, ex) in (1..=8).zip([1, 1, 1, 1, 2, 2, 2, 2]) {
    assert_eq!(reduced_image_dimensions(0, h)[4].1, ex, "failed h: {h}");
  }
  // five
  for (w, ex) in (1..=8).zip([1, 1, 2, 2, 3, 3, 4, 4]) {
    assert_eq!(reduced_image_dimensions(w, 0)[5].0, ex, "failed w: {w}");
  }
  for (h, ex) in (1..=8).zip([0, 0, 1, 1, 1, 1, 2, 2]) {
    assert_eq!(reduced_image_dimensions(0, h)[5].1, ex, "failed h: {h}");
  }
  // six
  for (w, ex) in (1..=8).zip([0, 1, 1, 2, 2, 3, 3, 4]) {
    assert_eq!(reduced_image_dimensions(w, 0)[6].0, ex, "failed w: {w}");
  }
  for (h, ex) in (1..=8).zip([1, 1, 2, 2, 3, 3, 4, 4]) {
    assert_eq!(reduced_image_dimensions(0, h)[6].1, ex, "failed h: {h}");
  }
  // seven
  for (w, ex) in (1..=8).zip([1, 2, 3, 4, 5, 6, 7, 8]) {
    assert_eq!(reduced_image_dimensions(w, 0)[7].0, ex, "failed w: {w}");
  }
  for (h, ex) in (1..=8).zip([0, 1, 1, 2, 2, 3, 3, 4]) {
    assert_eq!(reduced_image_dimensions(0, h)[7].1, ex, "failed h: {h}");
  }
  //
  assert_eq!(
    reduced_image_dimensions(8, 8),
    [
      (8, 8), // zeroth
      (1, 1), // one
      (1, 1), // two
      (2, 1), // three
      (2, 2), // four
      (4, 2), // five
      (4, 4), // six
      (8, 4), // seven
    ]
  );
}
