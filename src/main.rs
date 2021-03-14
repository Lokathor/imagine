use imagine::*;

fn main() {
  let bytes = std::fs::read("test1.png").unwrap();

  for png_chunk in PngChunkIter::from_png_bytes(&bytes).unwrap() {
    println!("{:?}", png_chunk);
    println!("Actual CRC: {:?}", png_chunk.get_actual_crc());
  }

  // TODO: get temp memory requirements.

  // TODO: get final memory requirements.

  // TODO: decompress IDAT into temp memory.

  // TODO: un-filter temp memory into final memory.
}
