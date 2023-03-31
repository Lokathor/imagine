use imagine::bmp::nice_header::bmp_get_nice_header;

fn main() {
  // Generic args and file opening stuff
  let args: Vec<String> = std::env::args().collect();
  println!("ARGS: {args:?}");
  if args.len() < 2 {
    println!("run this with a filename to try and open that file.");
    return;
  }
  let path = std::path::Path::new(&args[1]);
  print!("Reading `{}`... ", path.display());
  let bytes = match std::fs::read(path) {
    Ok(bytes) => {
      println!("got {} bytes.", bytes.len());
      bytes
    }
    Err(e) => {
      println!("{e:?}");
      return;
    }
  };

  println!("{:?}", bmp_get_nice_header(&bytes));
}
