use imagine::*;

fn main() {
  let bytes = std::fs::read("test1.png").unwrap();

  for png_chunk in PngChunkIter::from_png_bytes(&bytes).unwrap() {
    println!("{:?}", png_chunk);
    println!("> Actual CRC: {:?}", png_chunk.get_actual_crc());
  }

  let header = PngHeader::from_ihdr_chunk(
    PngChunkIter::from_png_bytes(&bytes).unwrap().next().unwrap(),
  )
  .unwrap();
  println!("{:?}", header);
  println!(
    "> temp memory required: {:?}",
    header.get_temp_memory_requirements().unwrap()
  );
  println!("> RGBA8888 memory required: {:?}", header.width * header.height * 4);

  let mut temp_buffer = vec![0; header.get_temp_memory_requirements().unwrap()];
  let decompress_result = decompress_idat_to(&mut temp_buffer, &bytes);
  println!("decompression result: {:?}", decompress_result);
  println!("temp_buffer> {:?}", temp_buffer);

  // TODO: un-filter temp memory into final memory.
}
