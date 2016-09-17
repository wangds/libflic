//! Codec for chunk type 11 = FLI_COLOR64.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicResult,RasterMut};

/// Magic for a FLI_COLOR64 chunk - 64-Level Color.
///
/// This chunk is identical to FLI_COLOR256 except that the values for
/// the red, green and blue components are in the range of 0-63
/// instead of 0-255.
pub const FLI_COLOR64: u16 = 11;

/// Decode a FLI_COLOR64 chunk.
pub fn decode_fli_color64(src: &[u8], dst: &mut RasterMut)
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
        try!(r.read_exact(&mut dst.pal[start..end]));

        idx0 = end;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fli_color64;

    #[test]
    fn test_decode_fli_color64() {
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

        {
            let mut dst = RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
            let res = decode_fli_color64(&src, &mut dst);
            assert!(res.is_ok());
        }

        assert_eq!(&pal[0..33], &expected[..]);
    }
}
