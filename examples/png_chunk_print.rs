use imagine::png::{PngChunk, PngRawChunkIter};

fn main() {
  let args: Vec<String> = std::env::args().collect();
  println!("ARGS: {args:?}");
  for file_arg in args[1..].iter() {
    let path = std::path::Path::new(file_arg);
    print!("Reading `{}`... ", path.display());
    let bytes = match std::fs::read(path) {
      Ok(bytes) => {
        println!("got {} bytes.", bytes.len());
        bytes
      }
      Err(e) => {
        println!("{e:?}");
        continue;
      }
    };
    for (n, raw_chunk) in PngRawChunkIter::new(&bytes).enumerate() {
      let chunk_res = PngChunk::try_from(raw_chunk);
      println!("{n}: {chunk_res:?}");
    }
  }
}
