use super::*;

mod bit_source;
use bit_source::*;

mod tree_entry;
use tree_entry::*;

mod code_length_alphabet;
use code_length_alphabet::*;

mod lit_len_alphabet;
use lit_len_alphabet::*;

mod dist_alphabet;
use dist_alphabet::*;

mod fixed_huffman_tree;
use fixed_huffman_tree::*;

/// Return: the number of bytes written
pub fn decompress_idat_to(out: &mut [u8], png_bytes: &[u8]) -> PngResult<usize> {
  decompress_zlib_to(
    out,
    PngChunkIter::from_png_bytes(png_bytes)?
      .filter(|c| c.chunk_type == ChunkType::IDAT)
      .map(|c| c.chunk_data),
  )
}

/// Return: the number of bytes written
fn decompress_zlib_to<'b>(
  out: &mut [u8], mut slices: impl Iterator<Item = &'b [u8]>,
) -> PngResult<usize> {
  let mut cur_slice = slices.next().ok_or(PngError::UnexpectedEndOfInput)?;
  trace!("decompress_zlib_to> {:?}", cur_slice);
  //
  let (cmf, rest) = cur_slice.split_first().ok_or(PngError::UnexpectedEndOfInput)?;
  cur_slice = rest;
  let cm = cmf & 0b1111;
  let cinfo = cmf >> 4;
  trace!("CMF: |{cmf:08b}| cm: {cm}, cinfo: {cinfo}", cmf = cmf, cm = cm, cinfo = cinfo);
  if cm != 8 {
    return Err(PngError::IllegalCompressionMethod);
  }
  if cinfo > 7 {
    return Err(PngError::IllegalCompressionInfo);
  }
  //
  let (flg, rest) = cur_slice.split_first().ok_or(PngError::UnexpectedEndOfInput)?;
  cur_slice = rest;
  let _fcheck = 0b11111 & flg;
  let fdict = ((1 << 5) & flg) > 0;
  let _flevel = flg >> 6;
  trace!(
    "FLG: |{flg:08b}| fcheck: {fcheck}, fdict: {fdict}, flevel: {flevel}",
    flg = flg,
    fcheck = _fcheck,
    fdict = fdict,
    flevel = _flevel
  );
  let fcheck_pass = u16::from_be_bytes([*cmf, *flg]) % 31 == 0;
  trace!("fcheck is correct: {}", fcheck_pass);
  if !fcheck_pass {
    return Err(PngError::IllegalFlagCheck);
  }
  if fdict {
    return Err(PngError::IllegalFlagCheck);
  }

  let mut bit_source = BitSource::new(cur_slice, slices);
  decompress_deflate_to(out, &mut bit_source)
}

/// Return: the number of bytes written
#[allow(unused_labels)]
fn decompress_deflate_to<'b, I: Iterator<Item = &'b [u8]>>(
  out: &mut [u8], bit_src: &mut BitSource<'b, I>,
) -> PngResult<usize> {
  trace!("decompress_deflate_to> {:?}", bit_src);

  let mut out_position = 0;

  'per_block: loop {
    trace!("Begin Block Loop");

    trace!("{:?}", bit_src);
    let is_final_block = bit_src.next_bfinal()?;
    trace!("is_final_block: {:?}", is_final_block);

    trace!("{:?}", bit_src);
    let block_type = bit_src.next_btype()?;
    trace!("block_type: {:02b}", block_type);
    debug_assert!(block_type < 0b100);

    if block_type == 0b00 {
      trace!("uncompressed block");
      bit_src.flush_spare_bits();
      let len = bit_src.next_u16()?;
      let nlen = bit_src.next_u16()?;
      trace!("len: {:016b}, nlen: {:016b}", len, nlen);
      if !len != nlen {
        return Err(PngError::LenAndNLenDidNotMatch);
      }
      let mut len_u = len as usize;
      if out.len() < out_position + len_u {
        return Err(PngError::OutputOverflow);
      }
      let (_, mut new) = out.split_at_mut(out_position);
      out_position += len_u;
      while len_u > 0 {
        let x = bit_src.next_up_to_n_bytes(len_u);
        if x.is_empty() {
          return Err(PngError::UnexpectedEndOfInput);
        }
        debug_assert!(new.len() >= x.len());
        let (new_head, new_tail) = new.split_at_mut(x.len());
        new_head.copy_from_slice(x);
        new = new_tail;
        len_u -= x.len();
      }
    } else {
      let (lit_len_alphabet, dist_alphabet) = if block_type == 0b11 {
        const DYNAMIC_CODE_LENGTH_ORDER: &[usize] =
          &[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
        //
        trace!("reading dynamic tree data");
        let hlit = bit_src.next_bits_lsb(5)? + 257;
        let hdist = bit_src.next_bits_lsb(5)? + 1;
        let hclen = bit_src.next_bits_lsb(4)? + 4;
        let mut code_length_alphabet = CodeLengthAlphabet::default();
        for i in DYNAMIC_CODE_LENGTH_ORDER.iter().copied().take(hclen as usize) {
          code_length_alphabet.tree[i].bit_count = bit_src.next_bits_lsb(3)? as u16;
        }
        code_length_alphabet.refresh()?;
        //
        let mut litlen_alphabet = LitLenAlphabet::default();
        code_length_alphabet.fill_a_tree(
          hlit as usize,
          &mut litlen_alphabet.tree,
          bit_src,
        )?;
        litlen_alphabet.refresh()?;
        //
        let mut dist_alphabet = DistAlphabet::default();
        code_length_alphabet.fill_a_tree(
          hdist as usize,
          &mut dist_alphabet.tree,
          bit_src,
        )?;
        dist_alphabet.refresh()?;
        //
        (litlen_alphabet, dist_alphabet)
      } else {
        trace!("using fixed tree data");
        let mut dist_alphabet = DistAlphabet::default();
        dist_alphabet.tree.iter_mut().for_each(|m| m.bit_count = 5);
        dist_alphabet.refresh()?;
        // FIXME: we should pre-compute the distance tree and also make that a const.
        (FIXED_HUFFMAN_TREE, dist_alphabet)
      };
      trace!("{:?}", bit_src);
      'per_symbol: loop {
        trace!("pre-lit-len: {:?}", bit_src);
        let lit_len = lit_len_alphabet.pull_and_match(bit_src)?;
        match lit_len {
          lit @ 0..=255 => {
            if out_position < out.len() {
              trace!("pushing literal '{}'", lit);
              out[out_position] = lit as u8;
              out_position += 1;
            } else {
              return Err(PngError::OutputOverflow);
            }
          }
          256 => break 'per_symbol,
          len_symbol => {
            debug_assert!(lit_len <= 285);
            let len = if len_symbol <= 264 {
              len_symbol - 254
            } else if len_symbol <= 284 {
              let num_extra_bits = (len_symbol - 261) / 4;
              ((len_symbol - 265) % 4 + 4)
                << num_extra_bits + 3 + bit_src.next_bits_lsb(num_extra_bits as u32)?
            } else {
              258
            };
            trace!("back ref len: {}", len);
            let dist = {
              let dist_sym = dist_alphabet.pull_and_match(bit_src)?;
              debug_assert!(dist_sym <= 29);
              if dist_sym <= 3 {
                dist_sym + 1
              } else if dist_sym <= 29 {
                let num_extra_bits = dist_sym / 2 - 1;
                ((dist_sym % 2 + 2) << num_extra_bits)
                  + 1
                  + bit_src.next_bits_lsb(num_extra_bits as u32)?
              } else {
                panic!("illegal dist sym from the dist_alphabet")
              }
            };
            trace!("back ref dist: {}", dist);
            trace!("pushing back ref: <{}, {}>", len, dist);
            let start_of_back_dist = out_position
              .checked_sub(dist)
              .ok_or(PngError::BackRefToBeforeOutputStart)?;
            trace!("start position of back ref: {}", start_of_back_dist);
            if out_position + len <= out.len() {
              //let mut d = start_of_back_dist;
              //(0..len).for_each(|_| {
              //  out[out_position] = out[d];
              //  out_position += 1;
              //  d += 1;
              //});
              out.copy_within(
                start_of_back_dist..(start_of_back_dist + len),
                out_position,
              );
              out_position += len;
            } else {
              return Err(PngError::OutputOverflow);
            }
          }
        }
      }
    }

    if is_final_block {
      return Ok(out_position);
    }
  }
}
