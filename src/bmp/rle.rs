//! The BMP format's run-length encoding system.

use super::*;

/// Run-length Encoded 8bpp operations.
///
/// RLE decoding always starts at (0,0), with the origin in the lower left.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum BmpRle8Op {
  /// A run of `count` entries, each of `index`.
  Run { count: NonZeroU8, index: u8 },
  /// x = 0, y += 1.
  Newline,
  /// End of the RLE sequence.
  EndOfBmp,
  /// Adjust the current position right and up as specified.
  Delta { right: u32, up: u32 },
  /// Output two raw index values.
  Raw2 { q: u8, w: u8 },
  /// Output two raw index values.
  Raw1 { q: u8 },
}

/// Iterate RLE encoded data, 8 bits per pixel
#[inline]
pub fn bmp_iter_rle8(image_bytes: &[u8]) -> impl Iterator<Item = BmpRle8Op> + '_ {
  // Now the MSDN docs get kinda terrible. They talk about "encoded" and
  // "absolute" mode, but whoever wrote that is bad at writing docs. What
  // we're doing is we'll pull off two bytes at a time from the pixel data.
  // Then we look at the first byte in a pair and see if it's zero or not.
  //
  // * If the first byte is **non-zero** it's the number of times that the second
  //   byte appears in the output. The second byte is an index into the palette,
  //   and you just put out that color and output it into the bitmap however many
  //   times.
  // * If the first byte is **zero**, it signals an "escape sequence" sort of
  //   situation. The second byte will give us the details:
  //   * 0: end of line
  //   * 1: end of bitmap
  //   * 2: "Delta", the *next* two bytes after this are unsigned offsets to the
  //     right and up of where the output should move to (remember that this mode
  //     always describes the BMP with a bottom-left origin).
  //   * 3+: "Absolute", The second byte gives a count of how many bytes follow
  //     that we'll output without repetition. The absolute output sequences
  //     always have a padding byte on the ending if the sequence count is odd, so
  //     we can keep pulling `[u8;2]` at a time from our data and it all works.
  let mut it = image_bytes.chunks_exact(2);
  let mut raw_count = 0_u8;
  core::iter::from_fn(move || {
    if raw_count > 0 {
      let (q, w): (u8, u8) = match it.next()? {
        [q, w] => (*q, *w),
        _ => unimplemented!(),
      };
      let out = if raw_count >= 2 { BmpRle8Op::Raw2 { q, w } } else { BmpRle8Op::Raw1 { q } };
      raw_count = raw_count.saturating_sub(2);
      Some(out)
    } else {
      let (a, b): (u8, u8) = match it.next()? {
        [a, b] => (*a, *b),
        _ => unimplemented!(),
      };
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle8Op::Run { count, index: b }),
        None => match b {
          0 => Some(BmpRle8Op::Newline),
          1 => Some(BmpRle8Op::EndOfBmp),
          2 => {
            let (right, up): (u8, u8) = match it.next()? {
              [right, up] => (*right, *up),
              _ => unimplemented!(),
            };
            Some(BmpRle8Op::Delta { right: u32::from(right), up: u32::from(up) })
          }
          x => {
            let (q, w): (u8, u8) = match it.next()? {
              [q, w] => (*q, *w),
              _ => unimplemented!(),
            };
            let out = BmpRle8Op::Raw2 { q, w };
            raw_count = x.saturating_sub(2);
            Some(out)
          }
        },
      }
    }
  })
}

/// Run-length Encoded 4bpp operations.
///
/// RLE decoding always starts at (0,0), with the origin in the lower left.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum BmpRle4Op {
  /// A run of `count` entries, each of `index`.
  Run { count: NonZeroU8, index_h: u8, index_l: u8 },
  /// x = 0, y += 1.
  Newline,
  /// End of the RLE sequence.
  EndOfBmp,
  /// Adjust the current position right and up as specified.
  Delta { right: u32, up: u32 },
  /// Output four raw index values.
  Raw4 { a: u8, b: u8, c: u8, d: u8 },
  /// Output three raw index values.
  Raw3 { a: u8, b: u8, c: u8 },
  /// Output two raw index values.
  Raw2 { a: u8, b: u8 },
  /// Output one raw index value.
  Raw1 { a: u8 },
}

/// Iterate RLE encoded data, 4 bits per pixel
#[inline]
pub fn bmp_iter_rle4(image_bytes: &[u8]) -> impl Iterator<Item = BmpRle4Op> + '_ {
  // RLE4 works *basically* how RLE8 does, except that every time we
  // process a byte as a color to output then it's actually two outputs
  // instead (upper bits then lower bits). The stuff about the escape
  // sequences and all that is still the same sort of thing.
  let mut it = image_bytes.chunks_exact(2);
  let mut raw_count = 0_u8;
  core::iter::from_fn(move || {
    if raw_count > 0 {
      let (q, w): (u8, u8) = match it.next()? {
        [q, w] => (*q, *w),
        _ => unimplemented!(),
      };
      let out = match raw_count {
        1 => BmpRle4Op::Raw1 { a: q >> 4 },
        2 => BmpRle4Op::Raw2 { a: q >> 4, b: q & 0b1111 },
        3 => BmpRle4Op::Raw3 { a: q >> 4, b: q & 0b1111, c: w >> 4 },
        _more => BmpRle4Op::Raw4 { a: q >> 4, b: q & 0b1111, c: w >> 4, d: w & 0b1111 },
      };
      raw_count = raw_count.saturating_sub(4);
      Some(out)
    } else {
      let (a, b): (u8, u8) = match it.next()? {
        [a, b] => (*a, *b),
        _ => unimplemented!(),
      };
      match NonZeroU8::new(a) {
        Some(count) => Some(BmpRle4Op::Run { count, index_h: b >> 4, index_l: b & 0b1111 }),
        None => match b {
          0 => Some(BmpRle4Op::Newline),
          1 => Some(BmpRle4Op::EndOfBmp),
          2 => {
            let (right, up): (u8, u8) = match it.next()? {
              [right, up] => (*right, *up),
              _ => unimplemented!(),
            };
            Some(BmpRle4Op::Delta { right: u32::from(right), up: u32::from(up) })
          }
          x => {
            let (q, w): (u8, u8) = match it.next()? {
              [q, w] => (*q, *w),
              _ => unimplemented!(),
            };
            let out = match raw_count {
              3 => BmpRle4Op::Raw3 { a: q >> 4, b: q & 0b1111, c: w >> 4 },
              _more => BmpRle4Op::Raw4 { a: q >> 4, b: q & 0b1111, c: w >> 4, d: w & 0b1111 },
            };
            raw_count = x.saturating_sub(4);
            Some(out)
          }
        },
      }
    }
  })
}
