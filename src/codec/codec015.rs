//! Codec for chunk type 15 = FLI_BRUN.

use std::cmp::min;
use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupByValue,LinScale};

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

/// Magic for a FPS_BRUN chunk - Postage Stamp, Byte Run Length Compression.
pub const FPS_BRUN: u16 = FLI_BRUN;

/// Decode a FLI_BRUN chunk.
pub fn decode_fli_brun(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);

    let start = dst.stride * dst.y;
    let end = dst.stride * (dst.y + dst.h);
    for row in dst.buf[start..end].chunks_mut(dst.stride) {
        let start = dst.x;
        let end = start + dst.w;
        let mut row = &mut row[start..end];
        let mut x0 = 0;

        // Skip obsolete count byte.
        let _count = r.read_u8()?;

        while x0 < row.len() {
            let signed_length = r.read_i8()? as i32;

            if signed_length >= 0 {
                let start = x0;
                let end = start + signed_length as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                let c = r.read_u8()?;
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

                r.read_exact(&mut row[start..end])?;

                x0 = end;
            }
        }
    }

    Ok(())
}

/// Decode a FPS_BRUN chunk.
pub fn decode_fps_brun(
        src: &[u8], src_w: usize, src_h: usize, dst: &mut RasterMut)
        -> FlicResult<()> {
    if src_w <= 0 || src_h <= 0 {
        return Err(FlicError::WrongResolution);
    }
    src_w.checked_mul(src_h).expect("overflow");

    let mut r = Cursor::new(src);
    let mut sy = 0;

    for (next_y, dy) in LinScale::new(src_h, dst.h) {
        // Handle case where src_y < dst.h.
        if next_y < sy && dy > 0 {
            let split = dst.stride * (dst.y + dy);
            let (src_row, dst_row) = dst.buf.split_at_mut(split);

            let src_start = dst.stride * (dst.y + dy - 1) + dst.x;
            let src_end = src_start + dst.w;
            let dst_start = dst.x;
            let dst_end = dst_start + dst.w;
            let src_row = &src_row[src_start..src_end];

            dst_row[dst_start..dst_end].copy_from_slice(src_row);
            continue;
        }

        while sy < next_y {
            decode_fps_brun_skip(&mut r, src_w)?;
            sy = sy + 1;
        }

        let start = dst.stride * (dst.y + dy) + dst.x;
        let end = start + dst.w;
        decode_fps_brun_line(&mut r, src_w, dst.w, &mut dst.buf[start..end])?;
        sy = sy + 1;
    }

    Ok(())
}

fn decode_fps_brun_skip(
        r: &mut Cursor<&[u8]>, sw: usize)
        -> FlicResult<()> {
    let mut sx = 0;

    // Skip obsolete count byte.
    let _count = r.read_u8()?;

    while sx < sw {
        let signed_length = r.read_i8()? as i32;

        if signed_length >= 0 {
            let end = sx + signed_length as usize;
            r.seek(SeekFrom::Current(1))?;

            sx = end;
        } else {
            let end = sx + (-signed_length) as usize;
            r.seek(SeekFrom::Current((-signed_length) as i64))?;

            sx = end;
        }
    }

    Ok(())
}

fn decode_fps_brun_line(
        r: &mut Cursor<&[u8]>, sw: usize, dw: usize, dst: &mut [u8])
        -> FlicResult<()> {
    let mut buf = [0; (-(::std::i8::MIN as i32)) as usize];
    let mut linscale = LinScale::new(sw, dw).peekable();
    let mut sx = 0;

    // Skip obsolete count byte.
    let _count = r.read_u8()?;

    while sx < sw {
        let signed_length = r.read_i8()? as i32;

        // Each iteration processes
        //
        //  src[sx .. sx + |signed_length|],
        //
        // and the corresponding elements in dst.
        debug_assert!(linscale.peek().map_or(true, |next| sx <= next.0));

        if signed_length >= 0 {
            let end = sx + signed_length as usize;
            let c = r.read_u8()?;

            // Know src[sx..(sx + signed_length)] = c.
            while let Some(&(next_sx, next_dx)) = linscale.peek() {
                if next_sx < end {
                    dst[next_dx] = c;
                    linscale.next();
                } else {
                    break;
                }
            }

            sx = end;
        } else {
            let end = sx + (-signed_length) as usize;
            r.read_exact(&mut buf[0..(-signed_length) as usize])?;

            // Know src[sx..(sx - signed_length)] = buf.
            while let Some(&(next_sx, next_dx)) = linscale.peek() {
                if next_sx < end {
                    dst[next_dx] = buf[next_sx - sx];
                    linscale.next();
                } else {
                    break;
                }
            }

            sx = end;
        }
    }

    Ok(())
}

/// Encode a FLI_BRUN chunk.
pub fn encode_fli_brun<W: Write + Seek>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let max_size = (next.w * next.h) as u64;
    let pos0 = w.seek(SeekFrom::Current(0))?;

    let start = next.stride * next.y;
    let end = next.stride * (next.y + next.h);
    for n in next.buf[start..end].chunks(next.stride) {
        let n = &n[next.x..(next.x + next.w)];
        let pos1 = w.seek(SeekFrom::Current(0))?;

        // Dummy initial state.
        let mut state = Group::Same(0, 0);
        let mut count = 0;

        // Reserve space for count.
        w.write_u8(0)?;

        for g in GroupByValue::new(n) {
            if let Some(new_state) = combine_packets(state, g) {
                state = new_state;
            } else {
                count = write_packet(state, count, n, w)?;
                state = g;
            }
        }

        count = write_packet(state, count, n, w)?;

        let pos2 = w.seek(SeekFrom::Current(0))?;
        if pos2 - pos0 > max_size {
            return Err(FlicError::ExceededLimit);
        }

        // If count fits, then fill it in.
        if count <= ::std::u8::MAX as usize {
            w.seek(SeekFrom::Start(pos1))?;
            w.write_u8(count as u8)?;
            w.seek(SeekFrom::Start(pos2))?;
        }
    }

    // If odd number, pad it to be even.
    let mut pos1 = w.seek(SeekFrom::Current(0))?;
    if (pos1 - pos0) % 2 == 1 {
        w.write_u8(0)?;
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
                w.write_i8(l as i8)?;
                w.write_u8(buf[idx])?;

                len = len - l;
                count = count + 1;
            }
        },
        Group::Diff(mut idx, mut len) => {
            let max = (-(::std::i8::MIN as i32)) as usize;
            while len > 0 {
                let l = min(len, max);
                w.write_i8((-(l as i32)) as i8)?;
                w.write_all(&buf[idx..(idx + l)])?;

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
            0x02,       // count 2
            3,    0xAB, // length 3
            (-4i8) as u8,   // length -4
            0x01, 0x23, 0x45, 0x67 ];

        let expected = [
            0xAB, 0xAB, 0xAB,
            0x01, 0x23, 0x45, 0x67 ];

        const SCREEN_W: usize = 7;
        const SCREEN_H: usize = 1;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fli_brun(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[..], &expected[..]);
    }

    #[test]
    fn test_decode_fps_brun_downscale() {
        let src = [
            0x02,       // count 2
            6,    0xAB, // length 6
            (-8i8) as u8,   // length -8
            0x01, 0x01, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67,

            0x01,       // count 1
            (-14i8) as u8,   // length -14
            0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB,
            0x01, 0x01, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67,

            0x05,       // count 5
            6,    0xAB, // length 6
            2,    0x01, // length 2
            2,    0x23, // length 2
            2,    0x45, // length 2
            2,    0x67, // length 2

            0x01,       // count 1
            (-14i8) as u8,   // length -14
            0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB,
            0x01, 0x01, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67 ];

        let expected = [
            0xAB, 0xAB, 0xAB, 0x01, 0x23, 0x45, 0x67,
            0xAB, 0xAB, 0xAB, 0x01, 0x23, 0x45, 0x67 ];

        const SCREEN_W: usize = 7;
        const SCREEN_H: usize = 2;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fps_brun(&src, 7 * 2, 2 * 2,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[..], &expected[..]);
    }

    #[test]
    fn test_decode_fps_brun_upscale() {
        let src = [
            0x02,       // count 2
            3,    0xAB, // length 3
            (-4i8) as u8,   // length -4
            0x01, 0x23, 0x45, 0x67 ];

        let expected = [
            0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB,
            0x01, 0x01, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67,
            0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB,
            0x01, 0x01, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67 ];

        const SCREEN_W: usize = 7 * 2;
        const SCREEN_H: usize = 1 * 2;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fps_brun(&src, 7, 1,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[..], &expected[..]);
    }

    #[test]
    fn test_encode_fli_brun() {
        let src = [
            0xAB, 0xAB, 0xAB,
            0x01, 0x23, 0x45, 0x67, 0x89 ];

        let expected = [
            5,          // count 5
            3,    0xAB, // length 3
            (-5i8) as u8,   // length -5
            0x01, 0x23, 0x45, 0x67, 0x89,
            127,  0x00, // length 127
            127,  0x00, // length 127
            58,   0x00, // length 58
            0x00 ];     // even

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 1;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * 256];
        buf[0..8].copy_from_slice(&src[..]);

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
        let res = encode_fli_brun(&next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }
}
