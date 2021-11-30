use imagine::{
  png::{
    critical_errors_only, decompress_idat_to_temp_storage, unfilter_decompressed_data, PngChunk,
    PngError, PngPixelFormat, RawPngChunkIter, IDAT,
  },
  RGB8, RGBA8,
};

fn main() {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  if let Err(e) = parse_me_a_png_yo(GLIDER_BIG_RAINBOW) {
    println!("Error: {:?}", e);
  } else {
    println!("Success!");
  }
}

fn parse_me_a_png_yo(png: &[u8]) -> Result<Vec<RGBA8>, PngError> {
  let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);

  let ihdr =
    it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;

  // TODO: support other pixel formats by automatically converting non-RGBA8 data
  // into RGBA8 data during the unfiltering op.
  assert_eq!(ihdr.pixel_format, PngPixelFormat::RGBA8);

  let mut temp_memory_buffer: Vec<u8> = vec![0; ihdr.temp_memory_requirement()];

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
  let mut vec = Vec::new();
  vec.resize((ihdr.width * ihdr.height) as usize, RGBA8::default());
  //
  unfilter_decompressed_data(ihdr, &mut &mut temp_memory_buffer, |x, y, data| {
    //println!("x: {x}, y: {y}, data: {data:?}", x = x, y = y, data = data);
    vec[(y * ihdr.width + x) as usize] = bytemuck::cast_slice(data)[0];
  })?;
  //
  Ok(vec)
}
