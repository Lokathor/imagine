
use pixels::{Error, Pixels, SurfaceTexture};
use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

use imagine::{png::*, RGBA8};

fn main() -> Result<(), Error> {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  let (rgba8, width, height) = match parse_me_a_png_yo(GLIDER_BIG_RAINBOW) {
    Ok((rgba8, width, height)) => (rgba8, width, height),
    Err(e) => panic!("Error: {:?}", e),
  };
  //
  let event_loop = EventLoop::new();
  let window = {
    let size = LogicalSize::new(width as f64, height as f64);
    WindowBuilder::new()
      .with_title("Demo window")
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
    Event::RedrawRequested(_) => {
      pixels.get_frame().copy_from_slice(bytemuck::cast_slice(&rgba8));
      if pixels.render().map_err(|e| println!("pixels.render() failed: {}", e)).is_err() {
        *control_flow = ControlFlow::Exit;
      }
      window.request_redraw();
    }
    Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
      *control_flow = ControlFlow::Exit;
    }
    _ => (),
  });
}

fn parse_me_a_png_yo(png: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), PngError> {
  let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);
  let ihdr =
    it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;

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
  Ok((vec, ihdr.width, ihdr.height))
}
