use super::*;

/// Background color.
///
/// RGB and Greyscale colors are always given as `u16` values. The actual color
/// selected should stay within the bit depth range of the rest of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(nonstandard_style)]
#[allow(missing_docs)]
pub enum bKGD {
  Greyscale { y: u16 },
  RGB { r: u16, g: u16, b: u16 },
  Index { i: u8 },
}
impl TryFrom<&[u8]> for bKGD {
  type Error = ();
  #[inline]
  fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
    Ok(match slice {
      [y0, y1] => bKGD::Greyscale { y: u16::from_be_bytes([*y0, *y1]) },
      [r0, r1, g0, g1, b0, b1] => bKGD::RGB {
        r: u16::from_be_bytes([*r0, *r1]),
        g: u16::from_be_bytes([*g0, *g1]),
        b: u16::from_be_bytes([*b0, *b1]),
      },
      [i] => bKGD::Index { i: *i },
      _ => return Err(()),
    })
  }
}
impl TryFrom<PngChunk<'_>> for bKGD {
  type Error = ();
  #[inline]
  fn try_from(value: PngChunk<'_>) -> Result<Self, Self::Error> {
    match value {
      PngChunk::bKGD(bkgd) => Ok(bkgd),
      _ => Err(()),
    }
  }
}
