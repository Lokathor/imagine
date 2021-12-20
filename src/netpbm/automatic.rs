use super::*;

use crate::{iter_1bpp_high_to_low, BulkenIter, RGBA8};
use bytemuck::try_cast_slice;

#[cfg(feature = "alloc")]
pub fn netpbm_automatic_rgba_image(netpbm: &[u8]) -> Result<(Vec<RGBA8>, u32, u32), NetpbmError> {
  //
  let (header, pixel_data) = netpbm_parse_header(netpbm)?;
  println!("header: {:?}", header);
  //
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
    NetpbmDataFormat::Ascii_Y_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      NetpbmAsciiU8Iter::new(pixel_data)
        .filter_map(|r| r.ok())
        .map(|u| if max != u8::MAX { (((u as f32) / image_max_f) * type_max_f) as u8 } else { u })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Ascii_Y_U16 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      NetpbmAsciiU16Iter::new(pixel_data)
        .filter_map(|r| r.ok())
        .map(|u| {
          if max != u16::MAX {
            (((u as f32) / image_max_f) * type_max_f) as u8
          } else {
            (u >> 8) as u8
          }
        })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Ascii_RGB_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(NetpbmAsciiU8Iter::new(pixel_data).filter_map(|r| r.ok()).map(|u| {
        if max != u8::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          u
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Ascii_RGB_U16 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(NetpbmAsciiU16Iter::new(pixel_data).filter_map(|r| r.ok()).map(|u| {
        if max != u16::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          (u >> 8) as u8
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Binary_Y_1bpp => {
      iter_1bpp_high_to_low(pixel_data)
        .map(|b| if b { RGBA8::BLACK } else { RGBA8::WHITE })
        .take(pixel_count)
        .for_each(|color| image.push(color));
    }
    NetpbmDataFormat::Binary_Y_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      pixel_data
        .iter()
        .copied()
        .map(|u| if max != u8::MAX { (((u as f32) / image_max_f) * type_max_f) as u8 } else { u })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Binary_Y_U16BE { max } => {
      let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      pixel_data_u16be
        .iter()
        .copied()
        .map(|u_arr| {
          let u = u16::from_be_bytes(u_arr);
          if max != u16::MAX {
            (((u as f32) / image_max_f) * type_max_f) as u8
          } else {
            (u >> 8) as u8
          }
        })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Binary_Y_F32BE { max } => {
      let pixel_data_f32be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      pixel_data_f32be
        .iter()
        .copied()
        .map(|f_arr| {
          let f = f32::from_be_bytes(f_arr);
          ((f / max) * type_max_f) as u8
        })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Binary_Y_F32LE { max } => {
      let pixel_data_f32le: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      pixel_data_f32le
        .iter()
        .copied()
        .map(|f_arr| {
          let f = f32::from_le_bytes(f_arr);
          ((f / max) * type_max_f) as u8
        })
        .take(pixel_count)
        .for_each(|y| image.push(RGBA8 { r: y, g: y, b: y, a: 255 }));
    }
    NetpbmDataFormat::Binary_YA_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 2>(pixel_data.iter().copied().map(|u| {
        if max != u8::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          u
        }
      }))
      .take(pixel_count)
      .for_each(|[y, a]| image.push(RGBA8 { r: y, g: y, b: y, a }));
    }
    NetpbmDataFormat::Binary_YA_U16BE { max } => {
      let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 2>(pixel_data_u16be.iter().copied().map(|u_arr| {
        let u = u16::from_be_bytes(u_arr);
        if max != u16::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          (u >> 8) as u8
        }
      }))
      .take(pixel_count)
      .for_each(|[y, a]| image.push(RGBA8 { r: y, g: y, b: y, a }));
    }
    NetpbmDataFormat::Binary_RGB_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(pixel_data.iter().copied().map(|u| {
        if max != u8::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          u
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Binary_RGB_U16BE { max } => {
      let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(pixel_data_u16be.iter().copied().map(|u_arr| {
        let u = u16::from_be_bytes(u_arr);
        if max != u16::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          (u >> 8) as u8
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Binary_RGB_F32BE { max } => {
      let pixel_data_f32be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(pixel_data_f32be.iter().copied().map(|f_arr| {
        let f = f32::from_be_bytes(f_arr);
        ((f / max) * type_max_f) as u8
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Binary_RGB_F32LE { max } => {
      let pixel_data_f32le: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 3>(pixel_data_f32le.iter().copied().map(|f_arr| {
        let f = f32::from_le_bytes(f_arr);
        ((f / max) * type_max_f) as u8
      }))
      .take(pixel_count)
      .for_each(|[r, g, b]| image.push(RGBA8 { r, g, b, a: 255 }));
    }
    NetpbmDataFormat::Binary_RGBA_U8 { max } => {
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 4>(pixel_data.iter().copied().map(|u| {
        if max != u8::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          u
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b, a]| image.push(RGBA8 { r, g, b, a }));
    }
    NetpbmDataFormat::Binary_RGBA_U16BE { max } => {
      let pixel_data_u16be: &[[u8; 2]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let image_max_f = max as f32;
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 4>(pixel_data_u16be.iter().copied().map(|u_arr| {
        let u = u16::from_be_bytes(u_arr);
        if max != u16::MAX {
          (((u as f32) / image_max_f) * type_max_f) as u8
        } else {
          (u >> 8) as u8
        }
      }))
      .take(pixel_count)
      .for_each(|[r, g, b, a]| image.push(RGBA8 { r, g, b, a }));
    }
    NetpbmDataFormat::Binary_RGBA_F32BE { max } => {
      let pixel_data_f32be: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 4>(pixel_data_f32be.iter().copied().map(|f_arr| {
        let f = f32::from_be_bytes(f_arr);
        ((f / max) * type_max_f) as u8
      }))
      .take(pixel_count)
      .for_each(|[r, g, b, a]| image.push(RGBA8 { r, g, b, a }));
    }
    NetpbmDataFormat::Binary_RGBA_F32LE { max } => {
      let pixel_data_f32le: &[[u8; 4]] = match try_cast_slice(pixel_data) {
        Ok(s) => s,
        Err(_) => return Err(NetpbmError::InsufficientBytes),
      };
      let type_max_f = u8::MAX as f32;
      BulkenIter::<_, 4>(pixel_data_f32le.iter().copied().map(|f_arr| {
        let f = f32::from_le_bytes(f_arr);
        ((f / max) * type_max_f) as u8
      }))
      .take(pixel_count)
      .for_each(|[r, g, b, a]| image.push(RGBA8 { r, g, b, a }));
    }
  }
  if image.len() < pixel_count {
    println!("We parsed only {} out of {} expected pixels!", image.len(), pixel_count);
    image.resize(pixel_count, RGBA8::BLACK);
  }
  Ok((image, header.width, header.height))
}
