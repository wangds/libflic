//! Codec for chunk type 7 = FLI_SS2.

use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupBySS2};

/// Magic for a FLI_SS2 chunk - Word Aligned Delta Compression.
///
/// This format contains the differences between consecutive frames.
/// This is the format most often used by Animator Pro for frames
/// other than the first frame of an animation.  It is similar to the
/// line coded delta (LC) compression, but is word oriented instead of
/// byte oriented.  The data is organized into lines and each line is
/// organized into packets.
///
/// The first word in the data following the chunk header contains the
/// number of lines in the chunk.  Each line can begin with some
/// optional words that are used to skip lines and set the last byte
/// in the line for animations with odd widths.  These optional words
/// are followed by a count of the packets in the line.  The line
/// count does not include skipped lines.
///
/// The high order two bits of the word is used to determine the
/// contents of the word.
///
///   Bit 15 | Bit 14 | Meaning
///  :------:|:------:| ----------------------------------------------
///      0   |    0   | The word contains the packet count.  The packets follow this word.  The packet count can be zero; this occurs when only the last pixel on a line changes.
///      1   |    0   | The low order byte is to be stored in the last byte of the current line.  The packet count always follows this word.
///      1   |    1   | The word contains a line skip count.  The number of lines skipped is given by the absolute value of the word.  This word can be followed by more skip counts, by a last byte word, or by the packet count.
///
/// The packets in each line are similar to the packets for the line
/// coded chunk.  The first byte of each packet is a column skip
/// count.  The second byte is a packet type.  If the packet type is
/// positive, the packet type is a count of words to be copied from
/// the packet to the animation image.  If the packet type is
/// negative, the packet contains one more word which is to be
/// replicated.  The absolute value of the packet type gives the
/// number of times the word is to be replicated.  The high and low
/// order byte in the replicated word do not necessarily have the same
/// value.
pub const FLI_SS2: u16 = 7;

#[derive(Clone,Copy,Debug)]
enum SS2Op {
    Skip(usize),
    Memset(usize, usize),
    Memcpy(usize, usize),
    SetEnd(usize),
}

/// Decode a FLI_SS2 chunk.
pub fn decode_fli_ss2(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);
    let mut y = 0;

    let mut h = r.read_u16::<LE>()?;
    while y < dst.h && h > 0 {
        let mut count = r.read_u16::<LE>()?;

        if (count & (1 << 15)) != 0 {
            if (count & (1 << 14)) != 0 {
                // Skip lines.
                y = y + (-((count as i16) as i32)) as usize;
                continue;
            } else {
                // Write last byte.
                let idx = dst.stride * (dst.y + y) + (dst.x + dst.w - 1);
                dst.buf[idx] = count as u8;

                count = r.read_u16::<LE>()?;
                if count == 0 {
                    y = y + 1;
                    h = h - 1;
                    continue;
                }
            }
        }

        let start = dst.stride * (dst.y + y);
        let end = dst.stride * (dst.y + y + 1);
        let mut row = &mut dst.buf[start..end];
        let mut x0 = dst.x;

        for _ in 0..count {
            let nskip = r.read_u8()? as usize;
            let signed_length = r.read_i8()? as i32;

            if signed_length >= 0 {
                let start = x0 + nskip;
                let end = start + 2 * signed_length as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                r.read_exact(&mut row[start..end])?;

                x0 = end;
            } else {
                let start = x0 + nskip;
                let end = start + 2 * (-signed_length) as usize;
                let c0 = r.read_u8()?;
                let c1 = r.read_u8()?;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                for e in &mut row[start..end].chunks_mut(2) {
                    e[0] = c0;
                    e[1] = c1;
                }

                x0 = end;
            }
        }

        y = y + 1;
        h = h - 1;
    }

    Ok(())
}

/// Encode a FLI_SS2 chunk.
pub fn encode_fli_ss2<W: Write + Seek>(
        prev: &Raster, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if (prev.w != next.w) || (prev.h != next.h) {
        return Err(FlicError::WrongResolution);
    }

    // Reserve space for line_count.
    let max_size = (next.w * next.h) as u64;
    let pos0 = w.seek(SeekFrom::Current(0))?;
    w.write_u16::<LE>(0)?;

    let prev_start = prev.stride * prev.y;
    let prev_end = prev.stride * (prev.y + prev.h);
    let next_start = next.stride * next.y;
    let next_end = next.stride * (next.y + next.h);

    let mut line_count = 0;
    let mut skip_count = 0;

    for (p, n) in prev.buf[prev_start..prev_end].chunks(prev.stride)
            .zip(next.buf[next_start..next_end].chunks(next.stride)) {
        let p = &p[prev.x..(prev.x + prev.w)];
        let n = &n[next.x..(next.x + next.w)];

        if &p[..] == &n[..] {
            skip_count = skip_count + 1;
            continue;
        }

        if line_count == ::std::u16::MAX {
            return Err(FlicError::ExceededLimit);
        }
        line_count = line_count + 1;

        if skip_count > 0 {
            let max = -((0b1100_0000_0000_0000u16) as i16); // max = +16384
            while skip_count > max as usize {
                w.write_i16::<LE>(-max)?;
                skip_count = skip_count - max as usize;
            }

            w.write_i16::<LE>(-(skip_count as i16))?;
            skip_count = 0;
        }

        let mut gs = GroupBySS2::new_ss2(p, n)
                .set_prepend_same_run()
                .set_ignore_final_same_run();
        let mut packets: Vec<SS2Op> = Vec::new();
        let mut state = SS2Op::Skip(0);
        let mut count = 0;

        while let Some(g) = gs.next() {
            if let Some(new_state) = combine_packets(state, g) {
                state = new_state;

                // If an odd skip length is combined into a memcpy
                // operation, force the memcpy length to be even.
                if let SS2Op::Memcpy(start, n) = new_state {
                    if n % 2 == 1 {
                        state = SS2Op::Memcpy(start, n + 1);
                        gs.idx = gs.idx + 1;
                    }
                }
            } else {
                let new_state = convert_packet(g);

                packets.push(state);

                // Insert Skip(0) between Memcpy and Memset operations.
                match (state, new_state) {
                    (SS2Op::Skip(_), _) => {},
                    (_, SS2Op::Skip(_)) => {},
                    (_, SS2Op::SetEnd(_)) => {},
                    _ => packets.push(SS2Op::Skip(0)),
                }

                state = new_state
            }
        }

        if let SS2Op::Skip(_) = state {
        } else if let SS2Op::SetEnd(idx) = state {
            // Note: this must be followed by a packet count word.
            w.write_u8(n[idx])?; // low byte
            w.write_u8(0b1000_0000)?; // high byte
        } else {
            packets.push(state);
        }

        // Reserve space for count.
        let pos1 = w.seek(SeekFrom::Current(0))?;
        w.write_i16::<LE>(0)?;

        for g in packets {
            count = write_packet(g, count, n, w)?;
        }

        assert!(count % 2 == 0);
        if count > 2 * ::std::i16::MAX as usize {
            return Err(FlicError::ExceededLimit);
        }

        let pos2 = w.seek(SeekFrom::Current(0))?;
        if pos2 - pos0 > max_size {
            return Err(FlicError::ExceededLimit);
        }

        w.seek(SeekFrom::Start(pos1))?;
        w.write_u16::<LE>((count / 2) as u16)?;
        w.seek(SeekFrom::Start(pos2))?;
    }

    // Length guaranteed to be even.
    let pos1 = w.seek(SeekFrom::Current(0))?;
    assert!((pos1 - pos0) % 2 == 0);

    // Fill in line count.
    w.seek(SeekFrom::Start(pos0))?;
    w.write_u16::<LE>(line_count)?;
    w.seek(SeekFrom::Start(pos1))?;

    Ok((pos1 - pos0) as usize)
}

fn combine_packets(s0: SS2Op, s1: Group)
        -> Option<SS2Op> {
    match (s0, s1) {
        (SS2Op::Skip(a), Group::Same(_, b)) => return Some(SS2Op::Skip(a + b)),

        // Skip followed by SetEnd operation.
        //
        // Since the SetEnd is stored separately, s0 is the final
        // operation.  We want to ignore the final skip operation.
        (SS2Op::Skip(_), Group::Diff(_, 1)) => return Some(convert_packet(s1)),

        (SS2Op::Skip(_), Group::Diff(..)) => return None,

        (SS2Op::SetEnd(_), _) => unreachable!(),
        (_, Group::Diff(_, 1)) => return None,

        // 1. Memset: length + data (2)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (SS2Op::Memset(idx, a), Group::Same(_, b)) =>
            if a + b < 4 {
                return Some(SS2Op::Memcpy(idx, a + b));
            },

        // 1. Memset: length + data (2)
        //    Skip:   length (1)
        //    Memset: length (1) + data (2)
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        (SS2Op::Memset(idx, a), Group::Diff(_, b)) =>
            if a + b <= 6 {
                return Some(SS2Op::Memcpy(idx, a + b));
            },

        // 1. Memcpy: length + data (a)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (SS2Op::Memcpy(idx, a), Group::Same(_, b)) =>
            if b < 2 {
                return Some(SS2Op::Memcpy(idx, a + b));
            },

        // 1. Memcpy: length + data (a)
        //    Skip:   length (1)
        //    Memset: length (1) + data (2)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (SS2Op::Memcpy(idx, a), Group::Diff(_, b)) =>
            if b < 6 {
                return Some(SS2Op::Memcpy(idx, a + b));
            },
    }

    // Don't combine s0 and s1 into a single operation.
    None
}

fn convert_packet(g: Group)
        -> SS2Op {
    match g {
        Group::Same(_, len) => SS2Op::Skip(len),
        Group::Diff(start, 1) => SS2Op::SetEnd(start),
        Group::Diff(start, len) => {
            assert!(len % 2 == 0);
            SS2Op::Memset(start, len)
        },
    }
}

fn write_packet<W: Write>(
        op: SS2Op, count: usize, buf: &[u8], w: &mut W)
        -> FlicResult<usize> {
    let mut count = count;
    match op {
        SS2Op::Skip(mut len) => {
            let max = ::std::u8::MAX as usize;
            while len > max {
                w.write_u8(max as u8)?;
                w.write_i8(0)?; // copy 0

                len = len - max;
                count = count + 2;
            }

            w.write_u8(len as u8)?;
            count = count + 1;
        },
        SS2Op::Memset(idx, mut len) => {
            assert!(len % 2 == 0);
            len = len / 2;
            let max = (-(::std::i8::MIN as i32)) as usize;
            while len > max {
                w.write_i8(max as i8)?;
                w.write_u8(buf[idx + 0])?;
                w.write_u8(buf[idx + 1])?;
                w.write_u8(0)?; // skip 0

                len = len - max;
                count = count + 2;
            }

            w.write_i8(-(len as i32) as i8)?;
            w.write_u8(buf[idx + 0])?;
            w.write_u8(buf[idx + 1])?;
            count = count + 1;
        },
        SS2Op::Memcpy(mut idx, mut len) => {
            assert!(len % 2 == 0);
            len = len / 2;
            let max = ::std::i8::MAX as usize;
            while len > max {
                w.write_i8(max as i8)?;
                w.write_all(&buf[idx..(idx + 2 * max)])?;
                w.write_u8(0)?; // skip 0

                idx = idx + max * 2;
                len = len - max;
                count = count + 2;
            }

            w.write_u8(len as u8)?;
            w.write_all(&buf[idx..(idx + 2 * len)])?;
            count = count + 1;
        },
        SS2Op::SetEnd(_) => unreachable!(),
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use ::{Raster,RasterMut};
    use super::*;

    #[test]
    fn test_decode_fli_ss2() {
        let src = [
            0x02, 0x00, // hh 2
            0x02, 0x00, // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            2, (-4i8) as u8,    // skip 2, length -4
            0xAB, 0xCD,
            0xFF, 0xFF, // count -1
            0xEE, 0x80, // bit15 = 1, bit14 = 0, data = 0xEE
            0x00, 0x00 ];   // count 0

        let expected = [
            0x00, 0x00, 0x00,
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            0x00, 0x00,
            0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fli_ss2(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[0..23], &expected[..]);
        assert_eq!(buf[(SCREEN_W * 2) + (SCREEN_W - 1)], 0xEE);
    }

    #[test]
    fn test_encode_fli_ss2() {
        let src1 = [
            0x00, 0x00, 0x00,
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE ];

        let src2 = [
            0x00, 0x00, 0x00,
            0x01, 0x12, 0x00, 0x34, 0x45, 0x56, 0x00, 0x78, 0x89, 0x90,
            0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xEE ];

        let src3 = [
            0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00 ];

        let expected = [
            0x03, 0x00, // hh 3
            0xFE, 0xFF, // skip 2 lines
            0xEE, 0x80, // bit15 = 1, bit14 = 0, data = 0xEE
            0x02, 0x00, // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            14, 2,      // skip 14, length 2
            0xAA, 0xBB, 0xCC, 0xDD,

            0xFF, 0xFF, // skip 1 line
            0xEE, 0x80, // bit15 = 1, bit14 = 0, data = 0xEE
            0x02, 0x00, // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x12, 0x00, 0x34, 0x45, 0x56, 0x00, 0x78, 0x89, 0x90,
            0, (-4i8) as u8,    // skip 0, length -4
            0xAB, 0xCD,

            0xFF, 0xFF, // skip 1 line
            0x02, 0x00, // count 2
            0, (-4i8) as u8,    // skip 0, length -4
            0xAB, 0xCD,
            8, 5,       // skip 8, length 5
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90 ];

        const SCREEN_W: usize = 32;
        const SCREEN_H: usize = 8;
        let buf1 = [0; SCREEN_W * SCREEN_H];
        let mut buf2 = [0; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * 256];
        buf2[(SCREEN_W * 2)..(SCREEN_W * 2 + 32)].copy_from_slice(&src1[..]);
        buf2[(SCREEN_W * 4)..(SCREEN_W * 4 + 32)].copy_from_slice(&src2[..]);
        buf2[(SCREEN_W * 6)..(SCREEN_W * 6 + 32)].copy_from_slice(&src3[..]);

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let prev = Raster::new(SCREEN_W, SCREEN_H, &buf1, &pal);
        let next = Raster::new(SCREEN_W, SCREEN_H, &buf2, &pal);
        let res = encode_fli_ss2(&prev, &next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }

    #[test]
    fn test_encode_fli_ss2_packet_count() {
        // skip 8, length -4, 0x63, 0x63
        let src = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63 ];

        const SCREEN_W: usize = 16 * (::std::i16::MAX as usize + 1);
        const SCREEN_H: usize = 1;
        let buf1 = vec![0; SCREEN_W * SCREEN_H];
        let mut buf2 = vec![0; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * 256];

        for x in buf2.chunks_mut(src.len()) {
            x.copy_from_slice(&src[..]);
        }

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let prev = Raster::new(SCREEN_W, SCREEN_H, &buf1, &pal);
        let next = Raster::new(SCREEN_W, SCREEN_H, &buf2, &pal);
        let res = encode_fli_ss2(&prev, &next, &mut enc);
        assert!(res.is_err());
    }
}
