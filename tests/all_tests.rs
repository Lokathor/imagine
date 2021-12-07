use std::path::{Path, PathBuf};

use imagine::png::{PngChunk, RawPngChunkIter};

/// Recursively walks over the `path` given, which must be a directory.
///
/// Your `op` is passed a [`PathBuf`] for each file found.
///
/// ## Panics
/// * If the path given is not a directory.
pub fn recursive_read_dir(path: impl AsRef<Path>, mut op: impl FnMut(PathBuf)) {
  use std::collections::VecDeque;
  //
  let path = path.as_ref();
  assert!(path.is_dir());
  // Note(Lokathor): Being *literally* recursive can blow out the stack for no
  // reason. Instead, we use a queue based system. Each loop pulls a dir out of
  // the queue and walks it.
  // * If we find a sub-directory that goes into the queue for later.
  // * Files get passed to the `op`
  // * Symlinks we check if they point to a Dir or File and act accordingly.
  //
  // REMINDER: if a symlink makes a loop on the file system then this will trap
  // us in an endless loop. That's the user's fault!
  let mut path_q = VecDeque::new();
  path_q.push_back(PathBuf::from(path));
  while let Some(path_buf) = path_q.pop_front() {
    match std::fs::read_dir(&path_buf) {
      Err(e) => eprintln!("Can't read_dir {path}: {e}", path = path_buf.display(), e = e),
      Ok(read_dir) => {
        for result_dir_entry in read_dir {
          match result_dir_entry {
            Err(e) => eprintln!("Error with dir entry: {e}", e = e),
            Ok(dir_entry) => match dir_entry.file_type() {
              Ok(ft) if ft.is_dir() => path_q.push_back(dir_entry.path()),
              Ok(ft) if ft.is_file() => op(dir_entry.path()),
              Ok(ft) if ft.is_symlink() => match dir_entry.metadata() {
                Ok(metadata) if metadata.is_dir() => path_q.push_back(dir_entry.path()),
                Ok(metadata) if metadata.is_file() => op(dir_entry.path()),
                Err(e) => eprintln!(
                  "Can't get metadata for symlink {path}: {e}",
                  path = dir_entry.path().display(),
                  e = e
                ),
                _ => eprintln!(
                  "Found symlink {path} but it's not a file or a directory.",
                  path = dir_entry.path().display()
                ),
              },
              Err(e) => eprintln!(
                "Can't get file type of {path}: {e}",
                path = dir_entry.path().display(),
                e = e
              ),
              _ => eprintln!(
                "Found dir_entry {path} but it's not a file, directory, or symlink.",
                path = dir_entry.path().display()
              ),
            },
          }
        }
      }
    }
  }
}

#[test]
fn png_test_images_do_not_panic() {
  let png_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("png");
  recursive_read_dir(png_folder, |path_buf| {
    println!("== Using File `{path_buf}`", path_buf = path_buf.display());
    let png: Vec<u8> = std::fs::read(path_buf.as_path()).unwrap();
    RawPngChunkIter::new(&png).map(PngChunk::try_from).for_each(|res| println!("{:?}", res));

    // TODO: we should expand the test to do a full parse of the image
  });
  //panic!("end");
}
