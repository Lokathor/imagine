//! Iterators over BMP data.

use super::*;

/// Iterate the palette indexes of the image bytes, based on the bit depth.
///
/// Only images with `bits_per_pixel` of 1, 4, or 8 use the palette.
///
/// ## Panics
/// * The `bits_per_pixel` must be in the range `1..=8`.
#[inline]
pub fn bmp_iter_pal_indexes_no_compression(
  image_bytes: &[u8], width: u32, bits_per_pixel: u16,
) -> impl Iterator<Item = u8> + '_ {
  assert!((1..=8).contains(&bits_per_pixel));
  let count = u32::from(bits_per_pixel);
  let padded_bytes_per_line = padded_bytes_per_line(width, bits_per_pixel).unwrap_or(4);
  image_bytes.chunks_exact(padded_bytes_per_line).flat_map(move |line| {
    line
      .iter()
      .copied()
      .flat_map(move |bits| U8BitIterHigh::from_count_and_bits(count, bits))
      .take(width.try_into().unwrap_or_default())
  })
}

/// Iterates 24bpp BGR data in the image bytes.
///
/// The encoding of the `u8` values depends on if the image is sRGB or not. If
/// the image is not sRGB then it's most likely linear values in each channel.
#[inline]
pub fn bmp_iter_bgr24(image_bytes: &[u8], width: u32) -> impl Iterator<Item = [u8; 3]> + '_ {
  let padded_bytes_per_line = padded_bytes_per_line(width, 24).unwrap_or(4);
  image_bytes.chunks(padded_bytes_per_line).flat_map(move |line| {
    line
      .chunks_exact(3)
      .map(|c| <[u8; 3]>::try_from(c).unwrap_or_default())
      .take(width.try_into().unwrap_or_default())
  })
}

/// Iterates 16-bits-per-pixel values using the RGB bitmasks given.
#[inline]
pub fn bmp_iter_bitmask16_rgb(
  image_bytes: &[u8], r_mask: u16, g_mask: u16, b_mask: u16, width: u32,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros().min(15);
  let g_shift = g_mask.trailing_zeros().min(15);
  let b_shift = b_mask.trailing_zeros().min(15);
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let padded_bytes_per_line = padded_bytes_per_line(width, 16).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r_bits = (u & r_mask) >> r_shift;
      let g_bits = (u & g_mask) >> g_shift;
      let b_bits = (u & b_mask) >> b_shift;
      let r = (r_bits as f32) / r_max_f32;
      let g = (g_bits as f32) / g_max_f32;
      let b = (b_bits as f32) / b_max_f32;
      r32g32b32_Sfloat { r, g, b }
    })
}

/// Iterates 16-bits-per-pixel values using the RGBA bitmasks given.
#[inline]
pub fn bmp_iter_bitmask16_rgba(
  image_bytes: &[u8], r_mask: u16, g_mask: u16, b_mask: u16, a_mask: u16, width: u32,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros().min(15);
  let g_shift = g_mask.trailing_zeros().min(15);
  let b_shift = b_mask.trailing_zeros().min(15);
  let a_shift = a_mask.trailing_zeros().min(15);
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let a_max = a_mask >> a_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let a_max_f32 = a_max as f32;
  let padded_bytes_per_line = padded_bytes_per_line(width, 16).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r_bits = (u & r_mask) >> r_shift;
      let g_bits = (u & g_mask) >> g_shift;
      let b_bits = (u & b_mask) >> b_shift;
      let a_bits = (u & a_mask) >> a_shift;
      let r = (r_bits as f32) / r_max_f32;
      let g = (g_bits as f32) / g_max_f32;
      let b = (b_bits as f32) / b_max_f32;
      let a = (a_bits as f32) / a_max_f32;
      r32g32b32a32_Sfloat { r, g, b, a }
    })
}

/// Iterates 32-bits-per-pixel linear values using the RGB bitmasks given.
///
/// You should use this *only* if some of the masks aren't 8 bits big.
#[inline]
pub fn bmp_iter_bitmask32_linear_rgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, width: u32,
) -> impl Iterator<Item = r32g32b32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros().min(31);
  let g_shift = g_mask.trailing_zeros().min(31);
  let b_shift = b_mask.trailing_zeros().min(31);
  let r_max = r_mask >> r_shift;
  let g_max = g_mask >> g_shift;
  let b_max = b_mask >> b_shift;
  let r_max_f32 = r_max as f32;
  let g_max_f32 = g_max as f32;
  let b_max_f32 = b_max as f32;
  let padded_bytes_per_line = padded_bytes_per_line(width, 32).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r_bits = (u & r_mask) >> r_shift;
      let g_bits = (u & g_mask) >> g_shift;
      let b_bits = (u & b_mask) >> b_shift;
      let r = (r_bits as f32) / r_max_f32;
      let g = (g_bits as f32) / g_max_f32;
      let b = (b_bits as f32) / b_max_f32;
      r32g32b32_Sfloat { r, g, b }
    })
}

/// Iterates 32-bits-per-pixel linear values using the RGBA bitmasks given.
///
/// You should use this *only* if some of the masks aren't 8 bits big.
#[inline]
pub fn bmp_iter_bitmask32_linear_rgba(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32, width: u32,
) -> impl Iterator<Item = r32g32b32a32_Sfloat> + '_ {
  let r_shift = r_mask.trailing_zeros().min(31);
  let g_shift = g_mask.trailing_zeros().min(31);
  let b_shift = b_mask.trailing_zeros().min(31);
  let a_shift = a_mask.trailing_zeros().min(31);
  let r_max_f32 = (r_mask >> r_shift) as f32;
  let g_max_f32 = (g_mask >> g_shift) as f32;
  let b_max_f32 = (b_mask >> b_shift) as f32;
  let a_max_f32 = (a_mask >> a_shift) as f32;
  let padded_bytes_per_line = padded_bytes_per_line(width, 32).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r_bits = (u & r_mask) >> r_shift;
      let g_bits = (u & g_mask) >> g_shift;
      let b_bits = (u & b_mask) >> b_shift;
      let a_bits = (u & a_mask) >> a_shift;
      let r = (r_bits as f32) / r_max_f32;
      let g = (g_bits as f32) / g_max_f32;
      let b = (b_bits as f32) / b_max_f32;
      let a = (a_bits as f32) / a_max_f32;
      r32g32b32a32_Sfloat { r, g, b, a }
    })
}

/// Iterates 32-bits-per-pixel sRGB using the RGB bitmasks given.
///
/// It's assumed that each mask is 8 bits big, results will be weird if this is
/// not the case.
#[inline]
pub fn bmp_iter_bitmask32_srgb(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, width: u32,
) -> impl Iterator<Item = r8g8b8_Srgb> + '_ {
  let r_shift = r_mask.trailing_zeros().min(31);
  let g_shift = g_mask.trailing_zeros().min(31);
  let b_shift = b_mask.trailing_zeros().min(31);
  let padded_bytes_per_line = padded_bytes_per_line(width, 32).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r = ((u & r_mask) >> r_shift) as u8;
      let g = ((u & g_mask) >> g_shift) as u8;
      let b = ((u & b_mask) >> b_shift) as u8;
      r8g8b8_Srgb { r, g, b }
    })
}

/// Iterates 32-bits-per-pixel sRGBA using the RGBA bitmasks given.
///
/// It's assumed that each mask is 8 bits big, results will be weird if this is
/// not the case.
#[inline]
pub fn bmp_iter_bitmask32_srgba(
  image_bytes: &[u8], r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32, width: u32,
) -> impl Iterator<Item = r8g8b8a8_Srgb> + '_ {
  let r_shift = r_mask.trailing_zeros().min(31);
  let g_shift = g_mask.trailing_zeros().min(31);
  let b_shift = b_mask.trailing_zeros().min(31);
  let a_shift = a_mask.trailing_zeros().min(31);
  let padded_bytes_per_line = padded_bytes_per_line(width, 32).unwrap_or(4);
  image_bytes
    .chunks_exact(padded_bytes_per_line)
    .flat_map(move |line| {
      line
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap_or_default()))
        .take(width.try_into().unwrap_or_default())
    })
    .map(move |u| {
      let r = ((u & r_mask) >> r_shift) as u8;
      let g = ((u & g_mask) >> g_shift) as u8;
      let b = ((u & b_mask) >> b_shift) as u8;
      let a = ((u & a_mask) >> a_shift) as u8;
      r8g8b8a8_Srgb { r, g, b, a }
    })
}
