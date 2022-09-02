use imagine::{pixels::RGBA8888, Image};
use pixels::{Pixels, SurfaceTexture};
use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

fn main() {
  // Generic args and file opening stuff
  let args: Vec<String> = std::env::args().collect();
  println!("ARGS: {args:?}");
  if args.len() < 2 {
    println!("run this with the name of a PNG to view the PNG.");
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

  // THIS IS THE COOL PART WHERE WE'RE USING THE LIBRARY TO PARSE A PNG
  let image = match Image::<RGBA8888>::try_from_png_bytes(&bytes) {
    Some(image) => image,
    None => {
      println!("Couldn't parse the file as a PNG.");
      return;
    }
  };

  // Generic "make it show up on the screen" stuff.
  let event_loop = EventLoop::new();
  let window = {
    let size = LogicalSize::new(image.width as f64, image.height as f64);
    WindowBuilder::new()
      .with_title("Hello PNG")
      .with_inner_size(size)
      .with_min_inner_size(size)
      .with_max_inner_size(size)
      .build(&event_loop)
      .unwrap()
  };
  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(image.width, image.height, surface_texture)
      .expect("Couldn't initialize `pixels` library.")
  };
  event_loop.run(move |event, _, control_flow| {
    // Draw the current frame
    match event {
      Event::RedrawRequested(_) => {
        let frame = pixels.get_frame();
        let png_bytes: &[u8] = bytemuck::cast_slice(&image.pixels);
        frame.copy_from_slice(png_bytes);
        if let Err(e) = pixels.render() {
          println!("Error during rendering: {e:?}");
          *control_flow = ControlFlow::Exit;
        }
      }
      Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
        *control_flow = ControlFlow::Exit;
      }
      _ => (),
    }
  });
}
