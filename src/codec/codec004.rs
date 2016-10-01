//! Codec for chunk type 4 = FLI_COLOR256.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};

/// Magic for a FLI_COLOR256 chunk - 256-Level Color.
///
/// The data in this chunk is organized in packets.  The first word
/// following the chunk header is a count of the number of packets in
/// the chunk.
///
/// Each packet consists of a one-byte color index skip count, a
/// one-byte color count and three bytes of color information for each
/// color defined.
///
/// At the start of the chunk, the color index is assumed to be zero.
/// Before processing any colors in a packet, the color index skip
/// count is added to the current color index.  The number of colors
/// defined in the packet is retrieved.  A zero in this byte indicates
/// 256 colors follow.  The three bytes for each color define the red,
/// green, and blue components of the color in that order.  Each
/// component can range from 0 (off) to 255 (full on).  The data to
/// change colors 2, 7, 8, and 9 would appear as follows:
///
/// ```text
///     2                       ; two packets
///     2,1,r,g,b               ; skip 2, change 1
///     4,3,r,g,b,r,g,b,r,g,b   ; skip 4, change 3
/// ```
pub const FLI_COLOR256: u16 = 4;

/// Decode a FLI_COLOR256 chunk.
pub fn decode_fli_color256(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);
    let mut idx0 = 0;

    let count = try!(r.read_u16::<LE>()) as usize;
    for _ in 0..count {
        let nskip = try!(r.read_u8()) as usize;
        let ncopy = match try!(r.read_u8()) {
            0 => 256 as usize,
            n => n as usize,
        };

        let start = idx0 + 3 * nskip;
        let end = start + 3 * ncopy;
        if end > dst.pal.len() {
            return Err(FlicError::Corrupted);
        }

        try!(r.read_exact(&mut dst.pal[start..end]));

        idx0 = end;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::*;

    #[test]
    fn test_decode_fli_color256() {
        let src = [
            0x02, 0x00, // count 2
            1, 2,       // skip 1, copy 2
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            3, 4,       // skip 3, copy 4
            0x2A, 0x2B, 0x2C, 0x3A, 0x3B, 0x3C, 0x4A, 0x4B, 0x4C, 0x5A, 0x5B, 0x5C ];

        let expected = [
            0x00, 0x00, 0x00,
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x2A, 0x2B, 0x2C, 0x3A, 0x3B, 0x3C, 0x4A, 0x4B, 0x4C, 0x5A, 0x5B, 0x5C,
            0x00, 0x00, 0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];

        let res = decode_fli_color256(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&pal[0..33], &expected[..]);
    }
}
