use super::*;

/// A parsed PNG chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
pub enum PngChunk<'b> {
  /// Image Header
  IHDR(IHDR),
  /// sRGB Info
  sRGB(sRGBIntent),
  /// Gamma value times 100,000.
  gAMA(u32),
  /// Palette
  PLTE(PLTE<'b>),
  /// Transparency
  tRNS(tRNS<'b>),
  /// Background color
  bKGD(bKGD),
  /// Image Data
  IDAT(IDAT<'b>),
  /// Image End
  IEND,
}
// * TODO: cHRM
// * TODO: sBIT
impl<'b> TryFrom<PngRawChunk<'b>> for PngChunk<'b> {
  type Error = PngRawChunk<'b>;
  #[inline]
  fn try_from(raw: PngRawChunk<'b>) -> Result<Self, Self::Error> {
    Ok(match raw.type_ {
      PngRawChunkType::IHDR => {
        // this can fail, so use `return` to avoid the outer Ok()
        return IHDR::try_from(raw.data).map(PngChunk::IHDR).map_err(|_| raw);
      }
      PngRawChunkType::PLTE => match bytemuck::try_cast_slice::<u8, [u8; 3]>(raw.data) {
        Ok(entries) => PngChunk::PLTE(PLTE::from(entries)),
        Err(_) => return Err(raw),
      },
      PngRawChunkType::tRNS => PngChunk::tRNS(tRNS::from(raw.data)),
      PngRawChunkType::bKGD => {
        // this can fail, so use `return` to avoid the outer Ok()
        return bKGD::try_from(raw.data).map(PngChunk::bKGD).map_err(|_| raw);
      }
      PngRawChunkType::sRGB => PngChunk::sRGB(match raw.data.get(0) {
        Some(0) => sRGBIntent::Perceptual,
        Some(1) => sRGBIntent::RelativeColorimetric,
        Some(2) => sRGBIntent::Saturation,
        Some(3) => sRGBIntent::AbsoluteColorimetric,
        _ => return Err(raw),
      }),
      PngRawChunkType::gAMA if raw.data.len() == 4 => {
        PngChunk::gAMA(u32::from_be_bytes(raw.data.try_into().unwrap()))
      }
      PngRawChunkType::IDAT => PngChunk::IDAT(IDAT::from(raw.data)),
      PngRawChunkType::IEND => PngChunk::IEND,
      _ => return Err(raw),
    })
  }
}
