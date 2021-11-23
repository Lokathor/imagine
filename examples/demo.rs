use imagine::{
  png::{critical_errors_only, PngChunk, PngError, RawPngChunkIter},
  RGB8, RGBA8,
};

fn main() {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  println!("{:?}", parse_me_a_png_yo(GLIDER_BIG_RAINBOW));
}

#[allow(unused)]
fn parse_me_a_png_yo(png: &[u8]) -> Result<Vec<RGBA8>, PngError> {
  let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);

  let ihdr =
    it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;

  let temp_memory_buffer: Vec<u8> = vec![0; ihdr.temp_memory_requirement()];

  let mut palette: Option<&[RGB8]> = None;
  //
  todo!()
}
