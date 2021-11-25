use imagine::{
  png::{
    critical_errors_only, decompress_idat_to_temp_storage, PngChunk, PngError, RawPngChunkIter,
    IDAT,
  },
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

  let mut temp_memory_buffer: Vec<u8> = vec![0; ihdr.temp_memory_requirement()];

  let mut palette: Option<&[RGB8]> = None;

  let mut idat_peek = it.peekable();
  loop {
    match idat_peek.peek() {
      Some(Ok(PngChunk::IDAT(_))) => break,
      None => return Err(PngError::NoIDATChunks),
      _ => {
        idat_peek.next();
      }
    }
  }
  let idat_slice_it = idat_peek.filter_map(|r_chunk| match r_chunk {
    Ok(PngChunk::IDAT(IDAT { data })) => Some(data),
    _ => None,
  });
  decompress_idat_to_temp_storage(&mut temp_memory_buffer, idat_slice_it)?;
  //
  todo!()
}
