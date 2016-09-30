//! Codec for chunk type 15 = FLI_BRUN.

use std::cmp::min;
use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupByValue};

/// Magic for a FLI_BRUN chunk - Byte Run Length Compression.
///
/// This chunk contains the entire image in a compressed format.
/// Usually this chunk is used in the first frame of an animation, or
/// within a postage stamp image chunk.
///
/// The data is organized in lines.  Each line contains packets of
/// compressed pixels.  The first line is at the top of the animation,
/// followed by subsequent lines moving downward.  The number of lines
/// in this chunk is given by the height of the animation.
///
/// The first byte of each line is a count of packets in the line.
/// This value is ignored, it is a holdover from the original
/// Animator.  It is possible to generate more than 255 packets on a
/// line.  The width of the animation is now used to drive the
/// decoding of packets on a line; continue reading and processing
/// packets until width pixels have been processed, then proceed to
/// the next line.
///
/// Each packet consist of a type/size byte, followed by one or more
/// pixels.  If the packet type is negative it is a count of pixels to
/// be copied from the packet to the animation image.  If the packet
/// type is positive it contains a single pixel which is to be
/// replicated; the absolute value of the packet type is the number of
/// times the pixel is to be replicated.
pub const FLI_BRUN: u16 = 15;

/// Decode a FLI_BRUN chunk.
pub fn decode_fli_brun(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);

    let start = dst.stride * dst.y;
    let end = dst.stride * (dst.y + dst.h);
    for row in dst.buf[start..end].chunks_mut(dst.stride) {
        // TODO: count value should be ignored, use width instead.
        let count = try!(r.read_u8());
        let mut x0 = dst.x;

        for _ in 0..count {
            let signed_length = try!(r.read_i8()) as i32;

            if signed_length >= 0 {
                let start = x0;
                let end = start + signed_length as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                let c = try!(r.read_u8());
                for e in &mut row[start..end] {
                    *e = c;
                }

                x0 = end;
            } else {
                let start = x0;
                let end = start + (-signed_length) as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                try!(r.read_exact(&mut row[start..end]));

                x0 = end;
            }
        }
    }

    Ok(())
}

/// Encode a FLI_BRUN chunk.
pub fn encode_fli_brun<W: Write + Seek>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let pos0 = try!(w.seek(SeekFrom::Current(0)));

    let start = next.stride * next.y;
    let end = next.stride * (next.y + next.h);
    for n in next.buf[start..end].chunks(next.stride) {
        let n = &n[next.x..(next.x + next.w)];
        let pos1 = try!(w.seek(SeekFrom::Current(0)));

        // Dummy initial state.
        let mut state = Group::Same(0, 0);
        let mut count = 0;

        // Reserve space for count.
        try!(w.write_u8(0));

        for g in GroupByValue::new(n) {
            if let Some(new_state) = combine_packets(state, g) {
                state = new_state;
            } else {
                count = try!(write_packet(state, count, n, w));
                state = g;
            }
        }

        count = try!(write_packet(state, count, n, w));

        // If count fits, then fill it in.
        if count <= ::std::u8::MAX as usize {
            let pos2 = try!(w.seek(SeekFrom::Current(0)));
            try!(w.seek(SeekFrom::Start(pos1)));
            try!(w.write_u8(count as u8));
            try!(w.seek(SeekFrom::Start(pos2)));
        }
    }

    // If odd number, pad it to be even.
    let mut pos1 = try!(w.seek(SeekFrom::Current(0)));
    if (pos1 - pos0) % 2 == 1 {
        try!(w.write_u8(0));
        pos1 = pos1 + 1;
    }

    Ok((pos1 - pos0) as usize)
}

fn combine_packets(s0: Group, s1: Group)
        -> Option<Group> {
    match (s0, s1) {
        (_, Group::Diff(..)) => unreachable!(),

        // Initialisation only.
        (Group::Same(0, 0), _) => Some(s1),

        // 1. Memset: length (1) + data (1)
        //    Memset: length (1) + data (1)
        //
        // 2. Memcpy: length (1) + data (a)
        //    Memcpy: data (b)
        (Group::Same(idx, a), Group::Same(_, b)) => {
            if 1 + a + b <= 4 {
                Some(Group::Diff(idx, a + b))
            } else {
                None
            }
        },

        // 1. Memcpy: length (1) + data (a)
        //    Memset: length (1) + data (1)
        //
        // 2. Memcpy: length (1) + data (a)
        //    Memcpy: data (b)
        (Group::Diff(idx, a), Group::Same(_, b)) => {
            if b <= 2 {
                Some(Group::Diff(idx, a + b))
            } else {
                None
            }
        },
    }
}

fn write_packet<W: Write>(
        g: Group, count: usize, buf: &[u8], w: &mut W)
        -> FlicResult<usize> {
    let mut count = count;
    match g {
        Group::Same(idx, mut len) => {
            let max = ::std::i8::MAX as usize;
            while len > 0 {
                let l = min(len, max);
                try!(w.write_i8(l as i8));
                try!(w.write_u8(buf[idx]));

                len = len - l;
                count = count + 1;
            }
        },
        Group::Diff(mut idx, mut len) => {
            let max = (-(::std::i8::MIN as i32)) as usize;
            while len > 0 {
                let l = min(len, max);
                try!(w.write_i8((-(l as i32)) as i8));
                try!(w.write_all(&buf[idx..(idx + l)]));

                idx = idx + l;
                len = len - l;
                count = count + 1;
            }
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
    fn test_decode_fli_brun() {
        let src = [
            0x02,   // count 2
            3,      // length 3
            0xAB,
            (-4i8) as u8,   // length -4
            0x01, 0x23, 0x45, 0x67 ];

        let expected = [
            0xAB, 0xAB, 0xAB,
            0x01, 0x23, 0x45, 0x67,
            0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 1;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];

        {
            let mut dst = RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
            let res = decode_fli_brun(&src, &mut dst);
            assert!(res.is_ok());
        }

        assert_eq!(&buf[0..8], &expected[..]);
    }

    #[test]
    fn test_encode_fli_brun() {
        let src = [
            0xAB, 0xAB, 0xAB,
            0x01, 0x23, 0x45, 0x67, 0x89 ];

        let expected = [
            5,          // count 5
            3,          // length 3
            0xAB,
            (-5i8) as u8,   // length -5
            0x01, 0x23, 0x45, 0x67, 0x89,
            127,  0x00, // length 127
            127,  0x00, // length 127
            58,   0x00, // length 59
            0x00 ];     // even

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 1;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * NUM_COLS];
        buf[0..8].copy_from_slice(&src[..]);

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
        let res = encode_fli_brun(&next, &mut enc);
        assert!(res.is_ok());

        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }
}
