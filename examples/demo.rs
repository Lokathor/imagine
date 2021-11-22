use imagine::png::{PngChunk, RawPngChunkIter, IDAT};

fn main() {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  for res_chunk in RawPngChunkIter::new(GLIDER_BIG_RAINBOW).map(PngChunk::try_from) {
    match res_chunk {
      Ok(PngChunk::IDAT(IDAT { data })) => {
        println!("Ok(IDAT(IDAT {{ data: [{} bytes] }}))", data.len())
      }
      otherwise => println!("{:?}", otherwise),
    }
  }
}
