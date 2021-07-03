use imagine::*;

use std::{
  collections::VecDeque,
  path::{Path, PathBuf},
};

fn main() {
  let mut temp_buffer = vec![0; 1000 * 1024 * 1024];

  if let Err(e) = debug_process_a_png_file("test1.png", &mut temp_buffer) {
    println!("test1.png: {:?}", e);
  }

  recursive_read_dir("D:\\art\\", |p| {
    if p.extension().is_none() || p.extension().unwrap().to_str().unwrap() != "png" {
      return;
    }
    if let Err(e) = debug_process_a_png_file(&p, &mut temp_buffer) {
      println!("{}: {:?}", p.display(), e);
    } else {
      //println!("{}: ok!", p.display());
    }
  });
}

#[allow(unused)]
fn debug_process_a_png_file<P: AsRef<Path>>(
  path: P, temp_buffer: &mut Vec<u8>,
) -> PngResult<()> {
  let bytes = std::fs::read(path).unwrap();

  for png_chunk in PngChunkIter::from_png_bytes(&bytes)? {
    trace!("{:?}", png_chunk);
    trace!("Actual CRC: {:?}", png_chunk.get_actual_crc());
  }

  let header = PngHeader::from_ihdr_chunk(
    PngChunkIter::from_png_bytes(&bytes)
      .unwrap()
      .next()
      .ok_or(PngError::UnexpectedEndOfInput)?,
  )?;
  trace!("Header: {:?}", header);
  let temp_memory_required = header.get_temp_memory_requirements()?;
  trace!("Temp memory required: {:?}", temp_memory_required);
  trace!(
    "RGBA8888 memory required: {:?}",
    (header.width as u128) * (header.height as u128) * 4
  );

  let decompress_result = decompress_idat_to(temp_buffer, &bytes);
  trace!("decompression result: {:?}", decompress_result);

  trace!("filtered bytes: {:?}", &temp_buffer[..temp_buffer.len().min(20)]);
  if temp_memory_required < temp_buffer.len() {
    trace!("Reconstructing...");
    let reconstruct_result =
      reconstruct_in_place(&mut temp_buffer[..temp_memory_required], header);
    trace!("reconstruct result: {:?}", reconstruct_result);
    trace!("reconstructed bytes: {:?}", &temp_buffer[..temp_buffer.len().min(20)]);
  } else {
    trace!("Can't unfilter the data, the temp buffer is wrong.")
  }

  Ok(())
}

/// Recursively walks over the `path` given, which must be a directory.
///
/// Your `op` is passed a [`PathBuf`] for each file found.
pub fn recursive_read_dir(path: impl AsRef<Path>, mut op: impl FnMut(PathBuf)) {
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
