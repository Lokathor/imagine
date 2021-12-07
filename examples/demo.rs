use std::{ffi::OsStr, mem::size_of};

use bytemuck::cast_slice;
use imagine::{bmp::*, png::*, RGB16_BE, RGB8, RGBA16_BE, RGBA8, Y16_BE, YA16_BE, YA8};
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

      let (new_rgba8, width, height) =
        match path_buf.extension().and_then(OsStr::to_str).unwrap_or_default() {
          "png" => match parse_me_a_png_yo(&file_bytes) {
            Ok((rgba8, width, height)) => (rgba8, width, height),
            Err(e) => {
              eprintln!("Err parsing `{path_buf}`: {e:?}", path_buf = path_buf.display(), e = e);
              return;
            }
          },
          "bmp" => match parse_me_a_bmp_yo(&file_bytes) {
            Ok((rgba8, width, height)) => (rgba8, width, height),
            Err(e) => {
              eprintln!("Err parsing `{path_buf}`: {e:?}", path_buf = path_buf.display(), e = e);
              return;
            }
          },
          _ => {
            eprintln!(
              "File `{path_buf}` has an unsupported file extension.",
              path_buf = path_buf.display()
            );
            return;
          }
        };

      if width > 32768 || height > 32768 {
        println!("new image dimensions ({}, {}) are too large!", width, height);
        return;
      }

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

// TODO: when we move the bmp parsing into the crate itself, delete this extra
// copy of this function.
pub fn try_split_off_byte_array<const N: usize>(bytes: &[u8]) -> Option<([u8; N], &[u8])> {
  if bytes.len() >= N {
    let (head, tail) = bytes.split_at(N);
    let a: [u8; N] = head.try_into().unwrap();
    Some((a, tail))
  } else {
    None
  }
}

fn parse_me_a_bmp_yo(bmp: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), BmpError> {
  println!("== Parsing a BMP...");

  let (file_header, rest) = BmpFileHeader::try_from_bytes(bmp)?;
  println!("file_header: {:?}", file_header);
  if file_header.total_file_size as usize != bmp.len()
    || !(COMMON_BMP_TAGS.contains(&file_header.tag))
  {
    println!("actual size: {}", bmp.len());
    return Err(BmpError::ThisIsProbablyNotABmpFile);
  }

  let (info_header, mut rest) = BmpInfoHeader::try_from_bytes(rest)?;
  println!("info_header: {info_header:?}", info_header = info_header);
  let compression = info_header.compression();
  let bits_per_pixel = info_header.bits_per_pixel() as usize;
  let width: usize = info_header.width().unsigned_abs() as usize;
  let height: usize = info_header.height().unsigned_abs() as usize;
  let pixel_count: usize = width.saturating_mul(height);
  let mut final_storage: Vec<RGBA8> = Vec::new();
  final_storage.try_reserve(pixel_count).map_err(|_| BmpError::AllocError)?;
  final_storage.resize(pixel_count, RGBA8::default());

  let [r_mask, g_mask, b_mask, a_mask] = match compression {
    BmpCompression::Bitfields => {
      let (a, new_rest) = try_split_off_byte_array::<{ size_of::<u32>() * 3 }>(rest)
        .ok_or(BmpError::InsufficientBytes)?;
      rest = new_rest;
      [
        u32::from_le_bytes(a[0..4].try_into().unwrap()),
        u32::from_le_bytes(a[4..8].try_into().unwrap()),
        u32::from_le_bytes(a[8..12].try_into().unwrap()),
        0,
      ]
    }
    BmpCompression::AlphaBitfields => {
      let (a, new_rest) = try_split_off_byte_array::<{ size_of::<u32>() * 4 }>(rest)
        .ok_or(BmpError::InsufficientBytes)?;
      rest = new_rest;
      [
        u32::from_le_bytes(a[0..4].try_into().unwrap()),
        u32::from_le_bytes(a[4..8].try_into().unwrap()),
        u32::from_le_bytes(a[8..12].try_into().unwrap()),
        u32::from_le_bytes(a[12..16].try_into().unwrap()),
      ]
    }
    // When bitmasks aren't specified, there's default RGB mask values based on
    // the bit depth, either 555 (16-bit) or 888 (32-bit).
    _ => match bits_per_pixel {
      16 => [0b11111 << 10, 0b11111 << 5, 0b11111, 0],
      32 => [0b11111111 << 16, 0b11111111 << 8, 0b11111111, 0],
      _ => [0, 0, 0, 0],
    },
  };
  println!(
    "bitmasks: [\n  r:{r_mask:032b}\n  g:{g_mask:032b}\n  b:{b_mask:032b}\n  a:{a_mask:032b}\n]",
    r_mask = r_mask,
    g_mask = g_mask,
    b_mask = b_mask,
    a_mask = a_mask
  );

  #[allow(unused_assignments)]
  let palette: Vec<RGBA8> = match info_header.palette_len() {
    0 => Vec::new(),
    count => {
      let mut v = Vec::new();
      v.try_reserve(count).map_err(|_| BmpError::AllocError)?;
      match info_header {
        BmpInfoHeader::Core(_) => {
          let bytes_needed = count * size_of::<[u8; 3]>();
          let (pal_slice, new_rest) = if rest.len() < bytes_needed {
            return Err(BmpError::InsufficientBytes);
          } else {
            rest.split_at(bytes_needed)
          };
          rest = new_rest;
          let pal_slice: &[[u8; 3]] = cast_slice(pal_slice);
          for [r, g, b] in pal_slice.iter().copied() {
            v.push(RGBA8 { r, g, b, a: 0xFF });
          }
        }
        _ => {
          let bytes_needed = count * size_of::<[u8; 4]>();
          let (pal_slice, new_rest) = if rest.len() < bytes_needed {
            return Err(BmpError::InsufficientBytes);
          } else {
            rest.split_at(bytes_needed)
          };
          rest = new_rest;
          let pal_slice: &[[u8; 4]] = cast_slice(pal_slice);
          for [r, g, b, a] in pal_slice.iter().copied() {
            v.push(RGBA8 { r, g, b, a });
          }
          if v.iter().copied().all(|c| c.a == 0) {
            v.iter_mut().for_each(|c| c.a = 0xFF);
          }
        }
      }
      v
    }
  };
  println!("palette: {palette:?}", palette = palette);

  let pixel_data_start_index: usize = file_header.pixel_data_offset as usize;
  let pixel_data_end_index: usize = pixel_data_start_index + info_header.pixel_data_len();
  let pixel_data = if bmp.len() < pixel_data_end_index {
    return Err(BmpError::InsufficientBytes);
  } else {
    &bmp[pixel_data_start_index..pixel_data_end_index]
  };

  match compression {
    BmpCompression::RgbNoCompression
    | BmpCompression::Bitfields
    | BmpCompression::AlphaBitfields => {
      let bits_per_line: usize =
        bits_per_pixel.saturating_mul(info_header.width().unsigned_abs() as usize);
      let no_padding_bytes_per_line: usize =
        (bits_per_line / 8) + (((bits_per_line % 8) != 0) as usize);
      let bytes_per_line: usize =
        ((no_padding_bytes_per_line / 4) + ((no_padding_bytes_per_line % 4) != 0) as usize) * 4;
      debug_assert!(no_padding_bytes_per_line <= bytes_per_line);
      debug_assert_eq!(bytes_per_line % 4, 0);
      if (pixel_data.len() % bytes_per_line) != 0
        || (pixel_data.len() / bytes_per_line) != (info_header.height().unsigned_abs() as usize)
      {
        return Err(BmpError::PixelDataIllegalLength);
      }
      //

      match bits_per_pixel {
        1 | 2 | 4 => {
          let (base_mask, base_down_shift) = match bits_per_pixel {
            1 => (0b1000_0000, 7),
            2 => (0b1100_0000, 6),
            4 => (0b1111_0000, 4),
            _ => unreachable!(),
          };
          let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
            while let Some((y, data_row)) = i.next() {
              let mut x = 0;
              for byte in data_row.iter().copied() {
                let mut mask: u8 = base_mask;
                let mut down_shift: usize = base_down_shift;
                while mask != 0 && x < width {
                  let pal_index = (byte & mask) >> down_shift;
                  let rgba8 = palette.get(pal_index as usize).copied().unwrap_or_default();
                  final_storage[(y * width + x) as usize] = rgba8;
                  //
                  mask >>= bits_per_pixel;
                  down_shift = down_shift.wrapping_sub(bits_per_pixel);
                  x += 1;
                }
              }
            }
          };
          if info_header.height() < 0 {
            per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
          } else {
            per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
          }
        }
        8 => {
          let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
            while let Some((y, data_row)) = i.next() {
              for (x, pal_index) in
                data_row[..no_padding_bytes_per_line].iter().copied().enumerate()
              {
                let rgba8 = palette.get(pal_index as usize).copied().unwrap_or_default();
                final_storage[(y * width + x) as usize] = rgba8;
              }
            }
          };
          if info_header.height() < 0 {
            per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate());
          } else {
            per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate());
          }
        }
        24 => {
          let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
            while let Some((y, data_row)) = i.next() {
              for (x, [r, g, b]) in
                cast_slice::<u8, [u8; 3]>(&data_row[..no_padding_bytes_per_line])
                  .iter()
                  .copied()
                  .enumerate()
              {
                let rgba8 = RGBA8 { r, g, b, a: 0xFF };
                final_storage[(y * width + x) as usize] = rgba8;
              }
            }
          };
          if info_header.height() < 0 {
            per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
          } else {
            per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
          }
        }
        16 => {
          let r_shift: u32 = if r_mask != 0 { r_mask.trailing_zeros() } else { 0 };
          let g_shift: u32 = if g_mask != 0 { g_mask.trailing_zeros() } else { 0 };
          let b_shift: u32 = if b_mask != 0 { b_mask.trailing_zeros() } else { 0 };
          let a_shift: u32 = if a_mask != 0 { a_mask.trailing_zeros() } else { 0 };
          let r_max: f32 = (r_mask >> r_shift) as f32;
          let g_max: f32 = (g_mask >> g_shift) as f32;
          let b_max: f32 = (b_mask >> b_shift) as f32;
          let a_max: f32 = (a_mask >> a_shift) as f32;
          //
          #[rustfmt::skip]
          let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
            while let Some((y, data_row)) = i.next() {
              for (x, data) in cast_slice::<u8, [u8; 2]>(&data_row[..no_padding_bytes_per_line])
                .iter()
                .copied()
                .enumerate()
              {
                // TODO: look at how SIMD this could be.
                let u = u16::from_le_bytes(data) as u32;
                let r: u8 = if r_mask != 0 { ((((u & r_mask) >> r_shift) as f32 / r_max) * 255.0) as u8 } else { 0 };
                let g: u8 = if g_mask != 0 { ((((u & g_mask) >> g_shift) as f32 / g_max) * 255.0) as u8 } else { 0 };
                let b: u8 = if b_mask != 0 { ((((u & b_mask) >> b_shift) as f32 / b_max) * 255.0) as u8 } else { 0 };
                let a: u8 = if a_mask != 0 { ((((u & a_mask) >> a_shift) as f32 / a_max) * 255.0) as u8 } else { 0xFF };
                let rgba8 = RGBA8 { r, g, b, a };
                final_storage[(y * width + x) as usize] = rgba8;
              }
            }
          };
          if info_header.height() < 0 {
            per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
          } else {
            per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
          }
        }
        32 => {
          let r_shift: u32 = if r_mask != 0 { r_mask.trailing_zeros() } else { 0 };
          let g_shift: u32 = if g_mask != 0 { g_mask.trailing_zeros() } else { 0 };
          let b_shift: u32 = if b_mask != 0 { b_mask.trailing_zeros() } else { 0 };
          let a_shift: u32 = if a_mask != 0 { a_mask.trailing_zeros() } else { 0 };
          let r_max: f32 = (r_mask >> r_shift) as f32;
          let g_max: f32 = (g_mask >> g_shift) as f32;
          let b_max: f32 = (b_mask >> b_shift) as f32;
          let a_max: f32 = (a_mask >> a_shift) as f32;
          //
          #[rustfmt::skip]
          let mut per_row_op = |i: &mut dyn Iterator<Item = (usize, &[u8])>| {
            while let Some((y, data_row)) = i.next() {
              for (x, data) in cast_slice::<u8, [u8; 4]>(&data_row[..no_padding_bytes_per_line])
                .iter()
                .copied()
                .enumerate()
              {
                // TODO: look at how SIMD this could be.
                let u = u32::from_le_bytes(data);
                let r: u8 = if r_mask != 0 { ((((u & r_mask) >> r_shift) as f32 / r_max) * 255.0) as u8 } else { 0 };
                let g: u8 = if g_mask != 0 { ((((u & g_mask) >> g_shift) as f32 / g_max) * 255.0) as u8 } else { 0 };
                let b: u8 = if b_mask != 0 { ((((u & b_mask) >> b_shift) as f32 / b_max) * 255.0) as u8 } else { 0 };
                let a: u8 = if a_mask != 0 { ((((u & a_mask) >> a_shift) as f32 / a_max) * 255.0) as u8 } else { 0xFF };
                let rgba8 = RGBA8 { r, g, b, a };
                final_storage[(y * width + x) as usize] = rgba8;
              }
            }
          };
          if info_header.height() < 0 {
            per_row_op(&mut pixel_data.chunks_exact(bytes_per_line).enumerate())
          } else {
            per_row_op(&mut pixel_data.rchunks_exact(bytes_per_line).enumerate())
          }
        }
        _ => return Err(BmpError::IllegalBitDepth),
      }
    }
    // TODO: implement this, the explanation of how this works is here:
    // https://docs.microsoft.com/en-us/windows/win32/gdi/bitmap-compression
    BmpCompression::RgbRLE4 => return Err(BmpError::ParserIncomplete),
    BmpCompression::RgbRLE8 => return Err(BmpError::ParserIncomplete),
    // Note(Lokathor): Uh, I guess "the entire file is inside the 'pixel_array'
    // data" or whatever? We need example files that use this compression before
    // we can begin to check out what's going on here.
    BmpCompression::Jpeg => return Err(BmpError::ParserIncomplete),
    BmpCompression::Png => return Err(BmpError::ParserIncomplete),
    // Note(Lokathor): probably we never need to support this until someone asks?
    BmpCompression::CmykNoCompression => return Err(BmpError::ParserIncomplete),
    BmpCompression::CmykRLE4 => return Err(BmpError::ParserIncomplete),
    BmpCompression::CmykRLE8 => return Err(BmpError::ParserIncomplete),
  }

  Ok((final_storage, width as u32, height as u32))
}
