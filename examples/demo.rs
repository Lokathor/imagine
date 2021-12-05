use imagine::{png::*, RGB8, RGBA8};
use pixels::{Error, Pixels, SurfaceTexture};
use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

#[allow(dead_code)]
fn main() -> Result<(), Error> {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");
  const TILES_SHEET: &[u8] = include_bytes!("tiles-sheet.png");
  const EXP2: &[u8] = include_bytes!("exp2_0.png");

  let (mut rgba8, width, height) = match parse_me_a_png_yo(EXP2) {
    Ok((rgba8, width, height)) => (rgba8, width, height),
    Err(e) => panic!("Error: {:?}", e),
  };
  //
  let event_loop = EventLoop::new();
  let window = {
    let size = LogicalSize::new(width as f64, height as f64);
    WindowBuilder::new()
      .with_title("imagine> demo window")
      .with_inner_size(size)
      .with_min_inner_size(size)
      .build(&event_loop)
      .unwrap()
  };

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(width, height, surface_texture)?
  };

  event_loop.run(move |event, _, control_flow| match event {
    Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
      *control_flow = ControlFlow::Exit;
    }
    Event::RedrawRequested(_) => {
      pixels.get_frame().copy_from_slice(bytemuck::cast_slice(&rgba8));
      if pixels.render().map_err(|e| println!("pixels.render() failed: {}", e)).is_err() {
        *control_flow = ControlFlow::Exit;
      }
      window.request_redraw();
    }
    Event::WindowEvent { event: WindowEvent::DroppedFile(path_buf), .. } => {
      let file_bytes = match std::fs::read(path_buf.as_path()) {
        Ok(bytes) => bytes,
        Err(e) => {
          eprintln!("Err opening `{path_buf}`: {e}", path_buf = path_buf.display(), e = e);
          return;
        }
      };

      let (new_rgba8, width, height) = match parse_me_a_png_yo(&file_bytes) {
        Ok((rgba8, width, height)) => (rgba8, width, height),
        Err(e) => {
          eprintln!("Err parsing `{path_buf}`: {e:?}", path_buf = path_buf.display(), e = e);
          return;
        }
      };

      rgba8 = new_rgba8;
      let size = LogicalSize::new(width as f64, height as f64);
      window.set_min_inner_size(Some(size));
      window.set_inner_size(size);
      pixels.resize_buffer(width, height);
      pixels.resize_surface(width, height);
    }
    _ => (),
  });
}

fn parse_me_a_png_yo(png: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), PngError> {
  let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);
  let ihdr =
    it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;
  println!("{:?}", ihdr);

  let idat_peek = it.peekable();
  let idat_slice_it = idat_peek.filter_map(|r_chunk| match r_chunk {
    Ok(PngChunk::IDAT(IDAT { data })) => Some(data),
    _ => None,
  });
  let mut temp_memory_buffer = vec![0; ihdr.temp_memory_requirement()];
  decompress_idat_to_temp_storage(&mut temp_memory_buffer, idat_slice_it)?;
  //
  let mut vec = Vec::new();
  vec.resize((ihdr.width * ihdr.height) as usize, RGBA8::default());
  //
  match ihdr.pixel_format {
    PngPixelFormat::RGBA8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        vec[(y * ihdr.width + x) as usize] = bytemuck::cast_slice(data)[0];
      })?
    }
    PngPixelFormat::RGB8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let rgb: RGB8 = bytemuck::cast_slice(data)[0];
        vec[(y * ihdr.width + x) as usize] = RGBA8 { r: rgb.r, g: rgb.g, b: rgb.b, a: 0xFF };
      })?
    }
    _ => return Err(PngError::Illegal_IHDR),
  }
  println!();
  //
  Ok((vec, ihdr.width, ihdr.height))
}
