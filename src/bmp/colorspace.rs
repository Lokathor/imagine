use super::*;
use crate::util::*;

/// Colorspace data for the BMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum BmpColorspace {
  /// The usual sRGB colorspace.
  Srgb,

  /// The windows default color space (On windows 10, this is also sRGB).
  WindowsDefault,

  /// A profile elsewhere is linked to (by name).
  LinkedProfile,

  /// A profile is embedded into the end of the bitmap itself.
  EmbeddedProfile,

  /// The colorspace is calibrated according to the info given.
  Calibrated { endpoints: CIEXYZTRIPLE, gamma_red: u32, gamma_green: u32, gamma_blue: u32 },

  /// The colorspace tag was unknown.
  ///
  /// In this case, the endpoints and gamma values are still kept for you, but
  /// the data might be nonsensical values (including possibly just zeroed).
  Unknown { endpoints: CIEXYZTRIPLE, gamma_red: u32, gamma_green: u32, gamma_blue: u32 },
}
impl From<[u8; 52]> for BmpColorspace {
  #[inline]
  fn from(a: [u8; 52]) -> Self {
    match u32_le(&a[0..4]) {
      LCS_CALIBRATED_RGB => BmpColorspace::Calibrated {
        endpoints: CIEXYZTRIPLE {
          red: CIEXYZ { x: u32_le(&a[4..8]), y: u32_le(&a[8..12]), z: u32_le(&a[12..16]) },
          green: CIEXYZ { x: u32_le(&a[16..20]), y: u32_le(&a[20..24]), z: u32_le(&a[24..28]) },
          blue: CIEXYZ { x: u32_le(&a[28..32]), y: u32_le(&a[32..36]), z: u32_le(&a[36..40]) },
        },
        gamma_red: u32_le(&a[40..44]),
        gamma_green: u32_le(&a[44..48]),
        gamma_blue: u32_le(&a[48..52]),
      },
      LCS_sRGB => BmpColorspace::Srgb,
      LCS_WINDOWS_COLOR_SPACE => BmpColorspace::WindowsDefault,
      PROFILE_LINKED => BmpColorspace::LinkedProfile,
      PROFILE_EMBEDDED => BmpColorspace::EmbeddedProfile,
      _ => BmpColorspace::Unknown {
        endpoints: CIEXYZTRIPLE {
          red: CIEXYZ { x: u32_le(&a[4..8]), y: u32_le(&a[8..12]), z: u32_le(&a[12..16]) },
          green: CIEXYZ { x: u32_le(&a[16..20]), y: u32_le(&a[20..24]), z: u32_le(&a[24..28]) },
          blue: CIEXYZ { x: u32_le(&a[28..32]), y: u32_le(&a[32..36]), z: u32_le(&a[36..40]) },
        },
        gamma_red: u32_le(&a[40..44]),
        gamma_green: u32_le(&a[44..48]),
        gamma_blue: u32_le(&a[48..52]),
      },
    }
  }
}
impl From<BmpColorspace> for [u8; 52] {
  #[inline]
  fn from(c: BmpColorspace) -> Self {
    let mut a = [0; 52];
    match c {
      BmpColorspace::Srgb => {
        a[0..4].copy_from_slice(LCS_sRGB.to_le_bytes().as_slice());
      }
      BmpColorspace::WindowsDefault => {
        a[0..4].copy_from_slice(LCS_WINDOWS_COLOR_SPACE.to_le_bytes().as_slice());
      }
      BmpColorspace::LinkedProfile => {
        a[0..4].copy_from_slice(PROFILE_LINKED.to_le_bytes().as_slice());
      }
      BmpColorspace::EmbeddedProfile => {
        a[0..4].copy_from_slice(PROFILE_EMBEDDED.to_le_bytes().as_slice());
      }
      BmpColorspace::Calibrated { endpoints, gamma_red, gamma_green, gamma_blue } => {
        a[0..4].copy_from_slice(LCS_CALIBRATED_RGB.to_le_bytes().as_slice());
        a[4..8].copy_from_slice(endpoints.red.x.to_le_bytes().as_slice());
        a[8..12].copy_from_slice(endpoints.red.y.to_le_bytes().as_slice());
        a[12..16].copy_from_slice(endpoints.red.z.to_le_bytes().as_slice());
        a[16..20].copy_from_slice(endpoints.green.x.to_le_bytes().as_slice());
        a[20..24].copy_from_slice(endpoints.green.y.to_le_bytes().as_slice());
        a[24..28].copy_from_slice(endpoints.green.z.to_le_bytes().as_slice());
        a[28..32].copy_from_slice(endpoints.blue.x.to_le_bytes().as_slice());
        a[32..36].copy_from_slice(endpoints.blue.y.to_le_bytes().as_slice());
        a[36..40].copy_from_slice(endpoints.blue.z.to_le_bytes().as_slice());
        a[40..44].copy_from_slice(gamma_red.to_le_bytes().as_slice());
        a[44..48].copy_from_slice(gamma_green.to_le_bytes().as_slice());
        a[48..52].copy_from_slice(gamma_blue.to_le_bytes().as_slice());
      }
      BmpColorspace::Unknown { endpoints, gamma_red, gamma_green, gamma_blue } => {
        // this is a made up value for unknown color spaces, it's hopefully not
        // gonna clash with anything else.
        a[0..4].copy_from_slice((u32::MAX - 1).to_le_bytes().as_slice());
        a[4..8].copy_from_slice(endpoints.red.x.to_le_bytes().as_slice());
        a[8..12].copy_from_slice(endpoints.red.y.to_le_bytes().as_slice());
        a[12..16].copy_from_slice(endpoints.red.z.to_le_bytes().as_slice());
        a[16..20].copy_from_slice(endpoints.green.x.to_le_bytes().as_slice());
        a[20..24].copy_from_slice(endpoints.green.y.to_le_bytes().as_slice());
        a[24..28].copy_from_slice(endpoints.green.z.to_le_bytes().as_slice());
        a[28..32].copy_from_slice(endpoints.blue.x.to_le_bytes().as_slice());
        a[32..36].copy_from_slice(endpoints.blue.y.to_le_bytes().as_slice());
        a[36..40].copy_from_slice(endpoints.blue.z.to_le_bytes().as_slice());
        a[40..44].copy_from_slice(gamma_red.to_le_bytes().as_slice());
        a[44..48].copy_from_slice(gamma_green.to_le_bytes().as_slice());
        a[48..52].copy_from_slice(gamma_blue.to_le_bytes().as_slice());
      }
    }
    a
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[allow(missing_docs)]
pub struct CIEXYZTRIPLE {
  pub red: CIEXYZ,
  pub green: CIEXYZ,
  pub blue: CIEXYZ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[allow(missing_docs)]
pub struct CIEXYZ {
  pub x: FXPT2DOT30,
  pub y: FXPT2DOT30,
  pub z: FXPT2DOT30,
}

/// Fixed point, 2.30
pub type FXPT2DOT30 = u32;
