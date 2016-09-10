//! codec012.rs
//!
//! Codec for chunk type 12 = FLI_LC.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicResult,RasterMut};

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
/// Each line begins with two bytes.  The first byte contains the
/// starting x position of the data on the line, and the second byte
/// the number of packets for the line.  Unlike BRUN compression, the
/// packet count is significant (because this compression method is
/// only used on 320x200 FLICs).
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
                try!(r.read_exact(&mut row[start..end]));

                x0 = end;
            } else {
                let start = x0 + nskip;
                let end = start + (-signed_length) as usize;
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

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fli_lc;

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
}
