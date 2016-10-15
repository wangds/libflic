//! Codec for chunk type 12 = FLI_LC.

use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupByLC};

/// Magic for a FLI_LC chunk - Byte Aligned Delta Compression.
///
/// This chunk contains the differences between the previous frame and
/// this frame.  This compression method was used by the original
/// Animator, but is not created by Animator Pro.  This type of chunk
/// can appear in an Animator Pro file, however, if the file was
/// originally created by Animator, then some (but not all) frames
/// were modified using Animator Pro.
///
/// The first 16-bit word following the chunk header contains the
/// position of the first line in the chunk.  This is a count of lines
/// (down from the top of the image) which are unchanged from the
/// prior frame.  The second 16-bit word contains the number of lines
/// in the chunk.  The data for the lines follows these two words.
///
/// Each line begins with a single byte that contains the number of
/// packets for the line.  Unlike BRUN compression, the packet count
/// is significant (because this compression method is only used on
/// 320x200 FLICs).
///
/// Each packet consists of a single byte column skip, followed by a
/// packet type/size byte.  If the packet type is positive it is a
/// count of pixels to be copied from the packet to the animation
/// image.  If the packet type is negative it contains a single pixel
/// which is to be replicated; the absolute value of the packet type
/// gives the number of times the pixel is to be replicated.
///
/// # Note
///
/// The negative/positive meaning of the packet type bytes in LC
/// compression is reversed from that used in BRUN compression.  This
/// gives better performance during playback.
pub const FLI_LC: u16 = 12;

#[derive(Clone,Copy,Debug)]
enum LcOp {
    Skip(usize),
    Memset(usize, usize),
    Memcpy(usize, usize),
}

/// Decode a FLI_LC chunk.
pub fn decode_fli_lc(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);
    let y0 = try!(r.read_u16::<LE>()) as usize;
    let hh = try!(r.read_u16::<LE>()) as usize;

    let start = dst.stride * (dst.y + y0);
    let end = dst.stride * (dst.y + y0 + hh);
    for row in dst.buf[start..end].chunks_mut(dst.stride) {
        let count = try!(r.read_u8());
        let mut x0 = dst.x;

        for _ in 0..count {
            let nskip = try!(r.read_u8()) as usize;
            let signed_length = try!(r.read_i8()) as i32;

            if signed_length >= 0 {
                let start = x0 + nskip;
                let end = start + signed_length as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                try!(r.read_exact(&mut row[start..end]));

                x0 = end;
            } else {
                let start = x0 + nskip;
                let end = start + (-signed_length) as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                let c = try!(r.read_u8());
                for e in &mut row[start..end] {
                    *e = c;
                }

                x0 = end;
            }
        }
    }

    Ok(())
}

/// Encode a FLI_LC chunk.
pub fn encode_fli_lc<W: Write + Seek>(
        prev: &Raster, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if (prev.w != next.w) || (prev.h != next.h) {
        return Err(FlicError::WrongResolution);
    }

    let prev_start = prev.stride * prev.y;
    let prev_end = prev.stride * (prev.y + prev.h);
    let next_start = next.stride * next.y;
    let next_end = next.stride * (next.y + next.h);

    let y0 = prev.buf[prev_start..prev_end].chunks(prev.stride)
            .zip(next.buf[next_start..next_end].chunks(next.stride))
            .take_while(|&(p, n)| &p[prev.x..(prev.x + prev.w)] == &n[next.x..(next.x + next.w)])
            .count();

    if y0 >= next.h {
        return Ok(0);
    }

    let y1 = next.h - prev.buf[prev_start..prev_end].chunks(prev.stride)
            .zip(next.buf[next_start..next_end].chunks(next.stride))
            .rev()
            .take_while(|&(p, n)| &p[prev.x..(prev.x + prev.w)] == &n[next.x..(next.x + next.w)])
            .count();

    if y1 <= y0 {
        return Ok(0);
    }

    let hh = y1 - y0;
    if (y0 > ::std::u16::MAX as usize) || (hh > ::std::u16::MAX as usize) {
        return Err(FlicError::ExceededLimit);
    }

    // Reserve space for y0, hh.
    let max_size = (next.w * next.h) as u64;
    let pos0 = try!(w.seek(SeekFrom::Current(0)));
    try!(w.write_u16::<LE>(y0 as u16));
    try!(w.write_u16::<LE>(hh as u16));

    let prev_start = prev.stride * y0;
    let prev_end = prev.stride * y1;
    let next_start = next.stride * y0;
    let next_end = next.stride * y1;

    for (p, n) in prev.buf[prev_start..prev_end].chunks(prev.stride)
            .zip(next.buf[next_start..next_end].chunks(next.stride)) {
        let p = &p[prev.x..(prev.x + prev.w)];
        let n = &n[next.x..(next.x + next.w)];

        // Reserve space for count.
        let pos1 = try!(w.seek(SeekFrom::Current(0)));
        try!(w.write_u8(0));

        let mut state = LcOp::Skip(0);
        let mut count = 0;

        for g in GroupByLC::new_lc(p, n)
                .set_prepend_same_run()
                .set_ignore_final_same_run() {
            if let Some(new_state) = combine_packets(state, g) {
                state = new_state;
            } else {
                let new_state = convert_packet(g);

                count = try!(write_packet(state, count, n, w));

                // Insert Skip(0) between Memcpy and Memset operations.
                match (state, new_state) {
                    (LcOp::Skip(_), _) => {},
                    (_, LcOp::Skip(_)) => {},
                    _ => count = try!(write_packet(LcOp::Skip(0), count, n, w)),
                }

                if count > 2 * ::std::u8::MAX as usize {
                    return Err(FlicError::ExceededLimit);
                }

                state = new_state;
            }
        }

        if let LcOp::Skip(_) = state {
        } else {
            count = try!(write_packet(state, count, n, w));
        }

        assert!(count % 2 == 0);
        if count > 2 * ::std::u8::MAX as usize {
            return Err(FlicError::ExceededLimit);
        }

        let pos2 = try!(w.seek(SeekFrom::Current(0)));
        if pos2 - pos0 > max_size {
            return Err(FlicError::ExceededLimit);
        }

        try!(w.seek(SeekFrom::Start(pos1)));
        try!(w.write_u8((count / 2) as u8));
        try!(w.seek(SeekFrom::Start(pos2)));
    }

    // If odd number, pad it to be even.
    let mut pos1 = try!(w.seek(SeekFrom::Current(0)));
    if (pos1 - pos0) % 2 == 1 {
        try!(w.write_u8(0));
        pos1 = pos1 + 1;
    }

    Ok((pos1 - pos0) as usize)
}

fn combine_packets(s0: LcOp, s1: Group)
        -> Option<LcOp> {
    match (s0, s1) {
        (LcOp::Skip(a), Group::Same(_, b)) => return Some(LcOp::Skip(a + b)),
        (LcOp::Skip(_), Group::Diff(..)) => return None,

        // 1. Memset: length + data (1)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (LcOp::Memset(idx, a), Group::Same(_, b)) =>
            if a + b < 3 {
                return Some(LcOp::Memcpy(idx, a + b));
            },

        // 1. Memset: length + data (1)
        //    Skip:   length (1)
        //    Memset: length (1) + data (1)
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        (LcOp::Memset(idx, a), Group::Diff(_, b)) =>
            if a + b <= 4 {
                return Some(LcOp::Memcpy(idx, a + b));
            },

        // 1. Memcpy: length + data (a)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (LcOp::Memcpy(idx, a), Group::Same(_, b)) =>
            if b < 2 {
                return Some(LcOp::Memcpy(idx, a + b));
            },

        // 1. Memcpy: length + data (a)
        //    Skip:   length (1)
        //    Memset: length (1) + data (1)
        //    Skip:   length (1)
        //    Memcpy: length (1) + data
        //
        // 2. Memcpy: length + data (a)
        //    Memcpy: data (b)
        //    Memcpy: data
        (LcOp::Memcpy(idx, a), Group::Diff(_, b)) =>
            if b < 5 {
                return Some(LcOp::Memcpy(idx, a + b));
            },
    }

    // Don't combine s0 and s1 into a single operation.
    None
}

fn convert_packet(g: Group)
        -> LcOp {
    match g {
        Group::Same(_, len) => LcOp::Skip(len),
        Group::Diff(start, len) => LcOp::Memset(start, len),
    }
}

fn write_packet<W: Write>(
        op: LcOp, count: usize, buf: &[u8], w: &mut W)
        -> FlicResult<usize> {
    let mut count = count;
    match op {
        LcOp::Skip(mut len) => {
            let max = ::std::u8::MAX as usize;
            while len > max {
                try!(w.write_u8(max as u8));
                try!(w.write_i8(0)); // copy 0

                len = len - max;
                count = count + 2;
            }

            try!(w.write_u8(len as u8));
            count = count + 1;
        },
        LcOp::Memset(idx, mut len) => {
            let max = (-(::std::i8::MIN as i32)) as usize;
            while len > max {
                try!(w.write_i8(max as i8));
                try!(w.write_u8(buf[idx]));
                try!(w.write_u8(0)); // skip 0

                len = len - max;
                count = count + 2;
            }

            try!(w.write_i8(-(len as i32) as i8));
            try!(w.write_u8(buf[idx]));
            count = count + 1;
        },
        LcOp::Memcpy(mut idx, mut len) => {
            let max = ::std::i8::MAX as usize;
            while len > max {
                try!(w.write_i8(max as i8));
                try!(w.write_all(&buf[idx..(idx + max)]));
                try!(w.write_u8(0)); // skip 0

                idx = idx + max;
                len = len - max;
                count = count + 2;
            }

            try!(w.write_u8(len as u8));
            try!(w.write_all(&buf[idx..(idx + len)]));
            count = count + 1;
        },
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use ::{Raster,RasterMut};
    use super::*;

    #[test]
    fn test_decode_fli_lc() {
        let src = [
            0x02, 0x00, // y0 2
            0x01, 0x00, // hh 1
            0x02,       // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x23, 0x45, 0x67, 0x89,
            2, (-4i8) as u8,    // skip 2, length -4
            0xAB ];

        let expected = [
            0x00, 0x00, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89,
            0x00, 0x00, 0xAB, 0xAB, 0xAB, 0xAB,
            0x00, 0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];

        {
            let mut dst = RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
            let res = decode_fli_lc(&src, &mut dst);
            assert!(res.is_ok());
        }

        assert_eq!(&buf[(SCREEN_W * 2)..(SCREEN_W * 2 + 16)], &expected[..]);
    }

    #[test]
    fn test_encode_fli_lc() {
        let src = [
            0x00, 0x00,
            0x01, 0x23, 0x45, 0x67, 0x89,
            0x00, 0x00, 0xAB, 0xAB, 0xAB, 0xAB,
            0x00, 0x00 ];

        let expected = [
            0x02, 0x00, // y0 2
            0x01, 0x00, // hh 1
            2,          // count 2
            2, 5,       // skip 2, length 5
            0x01, 0x23, 0x45, 0x67, 0x89,
            2, (-4i8) as u8,    // skip 2, length -4
            0xAB,
            0x00];      // even

        const SCREEN_W: usize = 32;
        const SCREEN_H: usize = 4;
        const NUM_COLS: usize = 256;
        let buf1: Vec<u8> = vec![0; SCREEN_W * SCREEN_H];
        let mut buf2: Vec<u8> = vec![0; SCREEN_W * SCREEN_H];
        let pal: Vec<u8> = vec![0; 3 * NUM_COLS];
        buf2[(SCREEN_W * 2)..(SCREEN_W * 2 + 15)].copy_from_slice(&src[..]);

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let prev = Raster::new(SCREEN_W, SCREEN_H, &buf1, &pal);
        let next = Raster::new(SCREEN_W, SCREEN_H, &buf2, &pal);
        let res = encode_fli_lc(&prev, &next, &mut enc);
        assert!(res.is_ok());

        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }
}
