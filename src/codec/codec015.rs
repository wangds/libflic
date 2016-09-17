//! Codec for chunk type 15 = FLI_BRUN.

use std::io::{Cursor,Read};
use byteorder::ReadBytesExt;

use ::{FlicResult,RasterMut};

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
                let c = try!(r.read_u8());
                for e in &mut row[start..end] {
                    *e = c;
                }

                x0 = end;
            } else {
                let start = x0;
                let end = start + (-signed_length) as usize;
                try!(r.read_exact(&mut row[start..end]));

                x0 = end;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fli_brun;

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
}