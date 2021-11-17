use imagine::png::{PngChunk, RawPngChunkIter};

fn main() {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  for res_chunk in RawPngChunkIter::new(GLIDER_BIG_RAINBOW).map(PngChunk::try_from) {
    println!("Parsed: {:?}", res_chunk);
  }
}
