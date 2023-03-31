use imagine::png::PngRawChunkIter;
use walkdir::WalkDir;

#[test]
#[cfg(feature = "png")]
fn test_RawPngChunkIter_no_panics() {
  // even totally random data should never panic the iterator!
  for _ in 0..10 {
    let v = super::rand_bytes(1024);
    for _ in PngRawChunkIter::new(&v) {
      //
    }
  }
}

#[test]
#[cfg(all(feature = "alloc", feature = "miniz_oxide"))]
#[cfg(FALSE)]
fn test_pngs_do_not_panic_decoder() {
  // iter ALL files in the test folder, even non-png files shouldn't panic it.

  use std::ffi::OsStr;

  use imagine::bitmap::Bitmap;
  use pixel_formats::r8g8b8a8_Unorm;
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
    let _image_result = Bitmap::<r8g8b8a8_Unorm>::try_from_png_bytes(&v);
    // Most test images are "hostile" so they naturally fail to parse.
    // However, the library shouldn't panic even with a hostile image.
  }
}
