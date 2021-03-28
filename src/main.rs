use imagine::*;

use std::path::Path;

fn main() {
  let mut temp_buffer = vec![0; 1000 * 1024 * 1024];

  if let Err(e) = debug_process_a_png_file("test1.png", &mut temp_buffer) {
    println!("test1.png: {:?}", e);
  }

  /*
  // quick check all the files
  for dir_entry in std::fs::read_dir("target/png/").unwrap().map(Result::unwrap) {
    let path_buf = dir_entry.path();
    if path_buf.extension().unwrap().to_str().unwrap() != "png" {
      continue;
    }
    if let Err(e) = debug_process_a_png_file(&path_buf, &mut temp_buffer) {
      println!("{}: {:?}", path_buf.display(), e);
    } else {
      println!("{}: ok!", path_buf.display());
    }
  }
  */
}

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
  )
  .ok_or(PngError::UnexpectedEndOfInput)?;
  trace!("Header: {:?}", header);
  trace!(
    "Temp memory required: {:?}",
    header.get_temp_memory_requirements().ok_or(PngError::InterlaceNotSupported)?
  );
  trace!(
    "RGBA8888 memory required: {:?}",
    (header.width as u128) * (header.height as u128) * 4
  );

  let decompress_result = decompress_idat_to(temp_buffer, &bytes)?;
  trace!("decompression result: {:?}", decompress_result);

  Ok(())
}
