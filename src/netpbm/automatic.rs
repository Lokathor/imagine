use core::mem::size_of;

use super::*;

use crate::{iter_1bpp_high_to_low, BulkenIter, RGBA8};
use bytemuck::{cast, try_cast_slice};
use wide::{f32x4, i32x4, u32x4};

pub fn netpbm_automatic_rgba_image(netpbm: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), NetpbmError> {
  let (header, pixel_data) = netpbm_parse_header(netpbm)?;
  let pixel_count = header.width.saturating_mul(header.height) as usize;
  let mut image: Vec<RGBA8> = Vec::new();
  image.try_reserve(pixel_count)?;
  //
  match header.data_format {
    NetpbmDataFormat::Ascii_Y_1bpp => {
      NetpbmAscii1bppIter::new(pixel_data)
        .filter_map(|r| r.ok())
        .map(|b| if b { RGBA8::BLACK } else { RGBA8::WHITE })
        .take(pixel_count)
        .for_each(|color| image.push(color));
    }
    NetpbmDataFormat::Ascii_Y_U8 { max: u8::MAX } => {
      // When values are the full u8 range, we don't need to re-scale.
      NetpbmAsciiU8Iter::new(pixel_data)
        .filter_map(|r| r.ok())
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Ascii_Y_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let mut u8_it = NetpbmAsciiU8Iter::new(pixel_data).filter_map(|r| r.ok());
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = u8_it.next() {
        let one = u8_it.next().unwrap_or_default();
        let two = u8_it.next().unwrap_or_default();
        let three = u8_it.next().unwrap_or_default();
        let y_raw = i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Ascii_Y_U16 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let mut u16_it = NetpbmAsciiU16Iter::new(pixel_data).filter_map(|r| r.ok());
      // since we're processing 4 pixels at a time, we might have a partial batch at
      // the end.
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = u16_it.next() {
        let one = u16_it.next().unwrap_or_default();
        let two = u16_it.next().unwrap_or_default();
        let three = u16_it.next().unwrap_or_default();
        let y_raw = i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16_u32) | (y_scaled_u << 8_u32) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Ascii_RGB_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let mut u8_it =
        NetpbmAsciiU8Iter::new(pixel_data).filter_map(|r| r.ok()).take(3 * pixel_count);
      while let Some(r_raw) = u8_it.next() {
        let g_raw = u8_it.next().unwrap_or_default();
        let b_raw = u8_it.next().unwrap_or_default();
        let a_raw = 255;
        let rgba_raw =
          i32x4::from([r_raw as i32, g_raw as i32, b_raw as i32, a_raw as i32]).round_float();
        let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
        let [r32, g32, b32, a32]: [u32; 4] = cast(rgba_scaled_f.round_int());
        image.push(RGBA8 { r: r32 as u8, g: g32 as u8, b: b32 as u8, a: a32 as u8 });
      }
    }
    NetpbmDataFormat::Ascii_RGB_U16 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let mut u16_it =
        NetpbmAsciiU16Iter::new(pixel_data).filter_map(|r| r.ok()).take(3 * pixel_count);
      while let Some(r_raw) = u16_it.next() {
        let g_raw = u16_it.next().unwrap_or_default();
        let b_raw = u16_it.next().unwrap_or_default();
        let a_raw = 255;
        let rgba_raw =
          i32x4::from([r_raw as i32, g_raw as i32, b_raw as i32, a_raw as i32]).round_float();
        let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
        let [r32, g32, b32, a32]: [u32; 4] = cast(rgba_scaled_f.round_int());
        image.push(RGBA8 { r: r32 as u8, g: g32 as u8, b: b32 as u8, a: a32 as u8 });
      }
    }
    NetpbmDataFormat::Binary_Y_1bpp => {
      iter_1bpp_high_to_low(pixel_data)
        .map(|b| if b { RGBA8::BLACK } else { RGBA8::WHITE })
        .take(pixel_count)
        .for_each(|color| image.push(color));
    }
    NetpbmDataFormat::Binary_Y_U8 { max: u8::MAX } => {
      pixel_data
        .iter()
        .copied()
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Binary_Y_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let mut u8_it = pixel_data.iter().copied().take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = u8_it.next() {
        let one = u8_it.next().unwrap_or_default();
        let two = u8_it.next().unwrap_or_default();
        let three = u8_it.next().unwrap_or_default();
        let y_raw = i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Binary_Y_U16BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let mut u16_it =
        pixel_data_u16be.iter().copied().map(|arr| u16::from_be_bytes(arr)).take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = u16_it.next() {
        let one = u16_it.next().unwrap_or_default();
        let two = u16_it.next().unwrap_or_default();
        let three = u16_it.next().unwrap_or_default();
        let y_raw = i32x4::from([zero as i32, one as i32, two as i32, three as i32]).round_float();
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Binary_Y_F32BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = f32x4::from(max);
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let pixel_data_u16be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let mut f32_it =
        pixel_data_u16be.iter().copied().map(|arr| f32::from_be_bytes(arr)).take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = f32_it.next() {
        let one = f32_it.next().unwrap_or_default();
        let two = f32_it.next().unwrap_or_default();
        let three = f32_it.next().unwrap_or_default();
        let y_raw = f32x4::from([zero, one, two, three]);
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Binary_Y_F32LE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = f32x4::from(max);
      let a_x4 = u32x4::from(u8::MAX as u32) << 24;
      let pixel_data_u16be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let mut f32_it =
        pixel_data_u16be.iter().copied().map(|arr| f32::from_le_bytes(arr)).take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some(zero) = f32_it.next() {
        let one = f32_it.next().unwrap_or_default();
        let two = f32_it.next().unwrap_or_default();
        let three = f32_it.next().unwrap_or_default();
        let y_raw = f32x4::from([zero, one, two, three]);
        let y_scaled_f = (y_raw / image_max) * channel_max;
        let y_scaled_u: u32x4 = cast(y_scaled_f.round_int());
        // little endian, so bytes are packed into lanes as BGRA
        let rgba_x4 = a_x4 | (y_scaled_u << 16) | (y_scaled_u << 8) | y_scaled_u /* << 0 */;
        let rgba_array: [RGBA8; 4] = cast(rgba_x4);
        rgba_array.iter().copied().take(pixels_remaining).for_each(|p| image.push(p));
        pixels_remaining = pixels_remaining.saturating_sub(4);
      }
    }
    NetpbmDataFormat::Binary_YA_U8 { max: u8::MAX } => {
      let pixel_data_ya: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      pixel_data_ya
        .iter()
        .copied()
        .take(pixel_count)
        .for_each(|[y, a]| image.push(RGBA8 { r: y, g: y, b: y, a }));
    }
    NetpbmDataFormat::Binary_YA_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_ya: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let mut ya_it = pixel_data_ya.iter().copied().take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some([y0, a0]) = ya_it.next() {
        let [y1, a1] = ya_it.next().unwrap_or_default();
        let ya_raw = i32x4::from([y0 as i32, a0 as i32, y1 as i32, a1 as i32]).round_float();
        let ya_scaled_f = (ya_raw / image_max) * channel_max;
        let [y0, a0, y1, a1]: [u32; 4] = cast(ya_scaled_f.round_int());
        image.push(RGBA8 { r: y0 as u8, g: y0 as u8, b: y0 as u8, a: a0 as u8 });
        if pixels_remaining >= 2 {
          image.push(RGBA8 { r: y1 as u8, g: y1 as u8, b: y1 as u8, a: a1 as u8 });
        }
        pixels_remaining = pixels_remaining.saturating_sub(2);
      }
    }
    NetpbmDataFormat::Binary_YA_U16BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_ya: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let mut ya_it = pixel_data_ya
        .iter()
        .copied()
        .map(|arr| {
          [
            u16::from_be_bytes(arr[0..2].try_into().unwrap()),
            u16::from_be_bytes(arr[2..4].try_into().unwrap()),
          ]
        })
        .take(pixel_count);
      let mut pixels_remaining = pixel_count;
      while let Some([y0, a0]) = ya_it.next() {
        let [y1, a1] = ya_it.next().unwrap_or_default();
        let ya_raw = i32x4::from([y0 as i32, a0 as i32, y1 as i32, a1 as i32]).round_float();
        let ya_scaled_f = (ya_raw / image_max) * channel_max;
        let [y0, a0, y1, a1]: [u32; 4] = cast(ya_scaled_f.round_int());
        image.push(RGBA8 { r: y0 as u8, g: y0 as u8, b: y0 as u8, a: a0 as u8 });
        if pixels_remaining >= 2 {
          image.push(RGBA8 { r: y1 as u8, g: y1 as u8, b: y1 as u8, a: a1 as u8 });
        }
        pixels_remaining = pixels_remaining.saturating_sub(2);
      }
    }
    NetpbmDataFormat::Binary_RGB_U8 { max: u8::MAX } => {
      let pixel_data_ya: &[[u8; 3]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      pixel_data_ya
        .iter()
        .copied()
        .take(pixel_count)
        .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: u8::MAX }));
    }
    NetpbmDataFormat::Binary_RGB_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; 3]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
      rgb_it.for_each(|[r, g, b]| {
        let rgb_raw = i32x4::from([r as i32, g as i32, b as i32, max as i32]).round_float();
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGB_U16BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<u16>() * 3]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
      rgb_it.for_each(|[r0, r1, g0, g1, b0, b1]| {
        let rgb_raw = i32x4::from([
          u16::from_be_bytes([r0, r1]) as i32,
          u16::from_be_bytes([g0, g1]) as i32,
          u16::from_be_bytes([b0, b1]) as i32,
          max as i32,
        ])
        .round_float();
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGB_F32BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<f32>() * 3]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgb_it = pixel_data_rgb
        .iter()
        .copied()
        .map(|a| {
          [
            f32::from_be_bytes(a[0..4].try_into().unwrap()),
            f32::from_be_bytes(a[4..8].try_into().unwrap()),
            f32::from_be_bytes(a[8..12].try_into().unwrap()),
          ]
        })
        .take(pixel_count);
      rgb_it.for_each(|[r, g, b]| {
        let rgb_raw = f32x4::from([r, g, b, max]);
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGB_F32LE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<f32>() * 3]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgb_it = pixel_data_rgb
        .iter()
        .copied()
        .map(|a| {
          [
            f32::from_le_bytes(a[0..4].try_into().unwrap()),
            f32::from_le_bytes(a[4..8].try_into().unwrap()),
            f32::from_le_bytes(a[8..12].try_into().unwrap()),
          ]
        })
        .take(pixel_count);
      rgb_it.for_each(|[r, g, b]| {
        let rgb_raw = f32x4::from([r, g, b, max]);
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGBA_U8 { max: u8::MAX } => {
      let pixel_data_ya: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      pixel_data_ya
        .iter()
        .copied()
        .take(pixel_count)
        .for_each(|[r, g, b, a]| image.push(RGBA8 { r, g, b, a }));
    }
    NetpbmDataFormat::Binary_RGBA_U8 { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgba_it = pixel_data_rgb.iter().copied().take(pixel_count);
      rgba_it.for_each(|[r, g, b, a]| {
        let rgb_raw = i32x4::from([r as i32, g as i32, b as i32, a as i32]).round_float();
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGBA_U16BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<u16>() * 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgb_it = pixel_data_rgb.iter().copied().take(pixel_count);
      rgb_it.for_each(|[r0, r1, g0, g1, b0, b1, a0, a1]| {
        let rgb_raw = i32x4::from([
          u16::from_be_bytes([r0, r1]) as i32,
          u16::from_be_bytes([g0, g1]) as i32,
          u16::from_be_bytes([b0, b1]) as i32,
          u16::from_be_bytes([a0, a1]) as i32,
        ])
        .round_float();
        let rgb_scaled_f = (rgb_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgb_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGBA_F32BE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<f32>() * 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgba_it = pixel_data_rgb
        .iter()
        .copied()
        .map(|a| {
          [
            f32::from_be_bytes(a[0..4].try_into().unwrap()),
            f32::from_be_bytes(a[4..8].try_into().unwrap()),
            f32::from_be_bytes(a[8..12].try_into().unwrap()),
            f32::from_be_bytes(a[12..16].try_into().unwrap()),
          ]
        })
        .take(pixel_count);
      rgba_it.for_each(|[r, g, b, a]| {
        let rgba_raw = f32x4::from([r, g, b, a]);
        let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgba_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
    NetpbmDataFormat::Binary_RGBA_F32LE { max } => {
      let channel_max = i32x4::from(u8::MAX as i32).round_float();
      let image_max = i32x4::from(max as i32).round_float();
      let pixel_data_rgb: &[[u8; size_of::<f32>() * 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let rgba_it = pixel_data_rgb
        .iter()
        .copied()
        .map(|a| {
          [
            f32::from_le_bytes(a[0..4].try_into().unwrap()),
            f32::from_le_bytes(a[4..8].try_into().unwrap()),
            f32::from_le_bytes(a[8..12].try_into().unwrap()),
            f32::from_le_bytes(a[12..16].try_into().unwrap()),
          ]
        })
        .take(pixel_count);
      rgba_it.for_each(|[r, g, b, a]| {
        let rgba_raw = f32x4::from([r, g, b, a]);
        let rgba_scaled_f = (rgba_raw / image_max) * channel_max;
        let [r, g, b, a]: [u32; 4] = cast(rgba_scaled_f.round_int());
        image.push(RGBA8 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 });
      });
    }
  }
  if image.len() < pixel_count {
    image.resize(pixel_count, RGBA8::BLACK);
  }
  Ok((image, header.width, header.height))
}
