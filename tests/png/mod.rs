use imagine::png::RawPngChunkIter;
use walkdir::WalkDir;

#[test]
fn test_RawPngChunkIter_no_panics() {
  // iter ALL files in the test folder, even non-png files shouldn't panic it.
  for entry in WalkDir::new("tests/").into_iter().filter_map(|e| e.ok()) {
    println!("{}", entry.path().display());
    let v = match std::fs::read(entry.path()) {
      Ok(v) => v,
      Err(e) => {
        println!("Error reading file: {e:?}");
        continue;
      }
    };
    for _ in RawPngChunkIter::new(&v) {
      //
    }
  }
  // even totally random data should never panic the iterator!
  for _ in 0..10 {
    let v = super::rand_bytes(1024);
    for _ in RawPngChunkIter::new(&v) {
      //
    }
  }
}
