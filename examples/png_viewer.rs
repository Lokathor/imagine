use bitfrob::u8_replicate_bits;
use imagine::png::{
  is_png_header_correct, png_get_header, png_get_idat, png_get_palette, PngColorType,
};
use pixels::{Pixels, SurfaceTexture};
use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

fn main() {
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
  if !is_png_header_correct(&bytes) {
    println!("not a PNG file.");
    return;
  }
  let ihdr = match png_get_header(&bytes) {
    Some(ihdr) => ihdr,
    None => {
      println!("No IHDR detected.");
      return;
    }
  };
  println!("ihdr: {ihdr:?}");
  let mut zlib_buffer = vec![0; ihdr.get_zlib_decompression_requirement()];
  println!("Zlib requires {} bytes to decompress.", zlib_buffer.len());
  match miniz_oxide::inflate::decompress_slice_iter_to_slice(
    &mut zlib_buffer,
    png_get_idat(&bytes),
    true,
    true,
  ) {
    Ok(decompression_count) => {
      if decompression_count < zlib_buffer.len() {
        println!("Probable Error: decompressed less data than expected.");
      }
    }
    Err(e) => {
      println!("Error during zlib decompression: {e:?}");
      //return;
    }
  }
  let mut final_buffer = vec![[0_u8; 4]; (ihdr.width * ihdr.height) as usize];
  let plte = png_get_palette(&bytes).unwrap_or(&[]);
  if let Err(_e) = ihdr.unfilter_decompressed_data(&mut zlib_buffer, |x, y, data| {
    //println!("x: {x}, y: {y}, data: {data:?}");
    if let Some(p) = final_buffer.get_mut(((y * ihdr.width) + x) as usize) {
      match ihdr.color_type {
        PngColorType::RGB => {
          let [r, g, b] = if ihdr.bit_depth == 16 {
            [data[0], data[2], data[4]]
          } else {
            [data[0], data[1], data[2]]
          };
          *p = [r, g, b, 255];
        }
        PngColorType::RGBA => {
          let [r, g, b, a] = if ihdr.bit_depth == 16 {
            [data[0], data[2], data[4], data[6]]
          } else {
            [data[0], data[1], data[2], data[3]]
          };
          *p = [r, g, b, a];
        }
        PngColorType::YA => {
          let [y, a] = if ihdr.bit_depth == 16 { [data[0], data[2]] } else { [data[0], data[1]] };
          // TODO: handle alpha
          *p = [y, y, y, a];
        }
        PngColorType::Y => {
          let y = if ihdr.bit_depth == 16 {
            data[0]
          } else {
            u8_replicate_bits(ihdr.bit_depth as u32, data[0])
          };
          *p = [y, y, y, 255];
        }
        PngColorType::Index => {
          let [r, g, b] = *plte.get(data[0] as usize).unwrap_or(&[0, 0, 0]);
          *p = [r, g, b, 255];
        }
      }
    } else {
      println!("Tried to write to a pixel out of bounds of the buffer: {x},{y}");
    }
  }) {
    println!("Error during unfiltering.");
  }

  let event_loop = EventLoop::new();
  let window = {
    let size = LogicalSize::new(ihdr.width as f64, ihdr.height as f64);
    WindowBuilder::new()
      .with_title("Hello PNG")
      .with_inner_size(size)
      .with_min_inner_size(size)
      .build(&event_loop)
      .unwrap()
  };
  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(ihdr.width, ihdr.height, surface_texture)
      .expect("Couldn't initialize `pixels` library.")
  };
  event_loop.run(move |event, _, control_flow| {
    // Draw the current frame
    match event {
      Event::RedrawRequested(_) => {
        let frame = pixels.get_frame();
        let png_bytes: &[u8] = bytemuck::cast_slice(&final_buffer);
        frame.copy_from_slice(png_bytes);
        if let Err(e) = pixels.render() {
          println!("Error during rendering: {e:?}");
          *control_flow = ControlFlow::Exit;
          return;
        }
      }
      Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
        *control_flow = ControlFlow::Exit;
        return;
      }
      _ => (),
    }
  });
}
