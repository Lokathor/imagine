use imagine::{png::*, RGB16_BE, RGB8, RGBA16_BE, RGBA8, Y16_BE, YA16_BE, YA8};
use pixels::{wgpu::Color, Error, Pixels, SurfaceTexture};
use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

// Work around for https://github.com/rust-windowing/winit/pull/2078
#[cfg(target_os = "macos")]
#[link(name = "ColorSync", kind = "framework")]
extern "C" {}

fn main() -> Result<(), Error> {
  const GLIDER_BIG_RAINBOW: &[u8] = include_bytes!("glider-big-rainbow.png");

  let (mut rgba8, width, height) = match parse_me_a_png_yo(GLIDER_BIG_RAINBOW) {
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
      .with_resizable(false)
      .build(&event_loop)
      .unwrap()
  };

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(width, height, surface_texture)?
  };
  let channel = 153 as f64 / 255 as f64;
  pixels.set_clear_color(Color { r: channel, g: channel, b: channel, a: 1.0 });

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

/// Note(Lokathor): This is this is an example of how you can turn PNG data into
/// a pile of RGBA8 pixels, along with a width and height. In the future this
/// will be improved to be more robust, and then eventually moved into the
/// library itself (behind an `alloc` feature).
fn parse_me_a_png_yo(png: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), PngError> {
  println!("== Parsing A PNG...");
  let mut it = RawPngChunkIter::new(png).map(PngChunk::try_from).filter(critical_errors_only);
  let ihdr =
    it.next().ok_or(PngError::NoChunksPresent)??.to_ihdr().ok_or(PngError::FirstChunkNotIHDR)?;
  println!("{:?}", ihdr);

  let mut palette: Option<&[RGB8]> = None;
  let mut transparency: Option<tRNS> = None;

  let idat_peek = it.peekable();
  let idat_slice_it = idat_peek.filter_map(|r_chunk| match r_chunk {
    Ok(PngChunk::IDAT(IDAT { data })) => Some(data),
    Ok(PngChunk::PLTE(PLTE { data })) => {
      println!("Found a Palette!");
      palette = Some(data);
      None
    }
    Ok(PngChunk::tRNS(t)) => {
      println!("Found Transparency!");
      transparency = Some(t);
      None
    }
    // TODO: support utilizing background chunks.
    Ok(PngChunk::iCCP(_)) => {
      println!("iCCP(iCCP {{ .. }})");
      None
    }
    Ok(other) => {
      println!("{:?}", other);
      None
    }
    _ => None,
  });
  let mut temp_memory_buffer = vec![0; ihdr.temp_memory_requirement()];
  decompress_idat_to_temp_storage(&mut temp_memory_buffer, idat_slice_it)?;
  //
  let mut final_storage = Vec::new();
  final_storage.resize((ihdr.width.saturating_mul(ihdr.height)) as usize, RGBA8::default());
  //
  match ihdr.pixel_format {
    // we already have all four channels
    PngPixelFormat::RGBA8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let rgba8: RGBA8 = bytemuck::cast_slice(data)[0];
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::RGBA16 => {
      // TODO: some day we might want to display the full 16-bit channels, WGPU
      // supports it, we think.
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let rgba16_be: RGBA16_BE = bytemuck::cast_slice(data)[0];
        let rgba8 = rgba16_be_to_rgba8(rgba16_be);
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }

    // with rgb only, it adds alpha as fully opaque
    PngPixelFormat::RGB8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let rgb8: RGB8 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = rgb8_to_rgba8(rgb8);
        if let Some(rgb8_trns_key) = transparency.and_then(tRNS::to_rgb8) {
          if rgb8 == rgb8_trns_key {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::RGB16 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let rgb16_be: RGB16_BE = bytemuck::cast_slice(data)[0];
        let mut rgba8 = rgb16_be_to_rgba8(rgb16_be);
        if let Some(rgb16_be_trns_key) = transparency.and_then(tRNS::to_rgb16_be) {
          if rgb16_be == rgb16_be_trns_key {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }

    // grayscale
    PngPixelFormat::Y1 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let y1 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y1_to_rgba8(y1);
        if let Some(y8_trns_key) = transparency.and_then(tRNS::to_y8) {
          if y1 == y8_trns_key.y {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::Y2 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let y2 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y2_to_rgba8(y2);
        if let Some(y8_trns_key) = transparency.and_then(tRNS::to_y8) {
          if y2 == y8_trns_key.y {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::Y4 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let y4 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y4_to_rgba8(y4);
        if let Some(y8_trns_key) = transparency.and_then(tRNS::to_y8) {
          if y4 == y8_trns_key.y {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::Y8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let y8 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y1_to_rgba8(y8);
        if let Some(y8_trns_key) = transparency.and_then(tRNS::to_y8) {
          if y8 == y8_trns_key.y {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::Y16 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let y16: Y16_BE = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y8_to_rgba8(y16.y[0]);
        if let Some(y16_trns_key) = transparency.and_then(tRNS::to_y16_be) {
          if y16 == y16_trns_key {
            rgba8.a = 0;
          }
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }

    // also grayscale, but now we already have an alpha value we keep
    PngPixelFormat::YA8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let ya8: YA8 = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y8_to_rgba8(ya8.y);
        rgba8.a = ya8.a;
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
    PngPixelFormat::YA16 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let ya16_be: YA16_BE = bytemuck::cast_slice(data)[0];
        let mut rgba8 = y8_to_rgba8(ya16_be.y[0]);
        rgba8.a = ya16_be.a[0];
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }

    // indexed color looks into the palette (or black)
    PngPixelFormat::I1 | PngPixelFormat::I2 | PngPixelFormat::I4 | PngPixelFormat::I8 => {
      unfilter_decompressed_data(ihdr, &mut temp_memory_buffer, |x, y, data| {
        let index = data[0] as usize;
        let rgb8 = palette
          .map(|pal| match pal.get(index) {
            Some(thing) => *thing,
            None => RGB8::default(),
          })
          .unwrap_or_default();
        let rgba8 = RGBA8 {
          r: rgb8.r,
          g: rgb8.g,
          b: rgb8.b,
          a: transparency
            .and_then(|trns| match trns {
              tRNS::Y { y } => y.to_be_bytes().get(index).copied(),
              tRNS::RGB { .. } => trns.rgb_to_index().unwrap().get(index).copied(),
              tRNS::Index { data } => data.get(index).copied(),
            })
            .unwrap_or(0xFF),
        };
        final_storage[(y * ihdr.width + x) as usize] = rgba8;
      })?
    }
  }
  //
  Ok((final_storage, ihdr.width, ihdr.height))
}

fn y1_to_rgba8(y1: u8) -> RGBA8 {
  let y2 = y1 | (y1 << 1);
  y2_to_rgba8(y2)
}

fn y2_to_rgba8(y2: u8) -> RGBA8 {
  let y4 = y2 | (y2 << 2);
  y4_to_rgba8(y4)
}

fn y4_to_rgba8(y4: u8) -> RGBA8 {
  let y8 = y4 | (y4 << 4);
  y8_to_rgba8(y8)
}

fn y8_to_rgba8(y8: u8) -> RGBA8 {
  let y = y8 as f32;
  RGBA8 { r: (0.299 * y) as u8, g: (0.587 * y) as u8, b: (0.114 * y) as u8, a: 0xFF }
}

fn rgb8_to_rgba8(rgb8: RGB8) -> RGBA8 {
  RGBA8 { r: rgb8.r, g: rgb8.g, b: rgb8.b, a: 0xFF }
}

fn rgba16_be_to_rgba8(rgba16_be: RGBA16_BE) -> RGBA8 {
  RGBA8 { r: rgba16_be.r[0], g: rgba16_be.g[0], b: rgba16_be.b[0], a: rgba16_be.a[0] }
}

fn rgb16_be_to_rgba8(rgb16_be: RGB16_BE) -> RGBA8 {
  RGBA8 { r: rgb16_be.r[0], g: rgb16_be.g[0], b: rgb16_be.b[0], a: 0xFF }
}
