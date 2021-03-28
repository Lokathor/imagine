use imagine::*;

use std::path::Path;

fn main() {
  let mut temp_buffer = vec![0; 1000 * 1024 * 1024];

  if let Err(e) = debug_process_a_png_file("test1.png", &mut temp_buffer) {
    println!("test1.png: {:?}", e);
  }

  // /*
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
  // */
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
