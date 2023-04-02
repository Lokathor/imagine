use walkdir::WalkDir;

#[test]
#[cfg(all(feature = "alloc"))]
fn test_files_do_not_panic_decoder() {
  // iter ALL files in the test folder, even non-png files shouldn't panic it.

  use imagine::{bmp::bmp_try_bitmap_rgba, Bitmap};
  use pixel_formats::{r32g32b32a32_Sfloat, r8g8b8a8_Unorm};
  use std::ffi::OsStr;

  for entry in WalkDir::new("tests/").into_iter().filter_map(|e| e.ok()) {
    if entry.file_type().is_dir() {
      continue;
    }
    println!("{}", entry.path().display());
    let v = match std::fs::read(entry.path()) {
      Ok(v) => v,
      Err(e) => {
        println!("Error reading file: {e:?}");
        continue;
      }
    };
    let _: Option<Bitmap> = bmp_try_bitmap_rgba(&v, true).ok();
    // Most test images are "hostile" so they naturally fail to parse.
    // However, the library shouldn't panic even with a hostile image.
  }
}
