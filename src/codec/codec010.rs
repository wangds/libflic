//! Codec for chunk type 10 = FLI_SBSRSC.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};

/// Magic for a FLI_SBSRSC chunk.
///
/// This is likely to be used by very old development FLICs only.
pub const FLI_SBSRSC: u16 = 10;

/// Decode a FLI_SBSRSC chunk.
///
/// The following logic only makes sense for 320x200 FLICs.
pub fn decode_fli_sbsrsc(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    if dst.x != 0 || dst.y != 0
            || dst.w != 320 || dst.h != 200 || dst.stride != 320 {
        return Err(FlicError::WrongResolution);
    }

    let mut r = Cursor::new(src);
    let mut idx0 = try!(r.read_u16::<LE>()) as usize;

    let count = try!(r.read_u16::<LE>());
    for _ in 0..count {
        let nskip = try!(r.read_u8()) as usize;
        let signed_length = try!(r.read_i8()) as i32;

        if signed_length >= 0 {
            let start = idx0 + nskip;
            let end = start + signed_length as usize;
            if end > dst.buf.len() {
                return Err(FlicError::Corrupted);
            }

            try!(r.read_exact(&mut dst.buf[start..end]));

            idx0 = end;
        } else {
            let start = idx0 + nskip;
            let end = start + (-signed_length) as usize;
            if end > dst.buf.len() {
                return Err(FlicError::Corrupted);
            }

            let c = try!(r.read_u8());
            for e in &mut dst.buf[start..end] {
                *e = c;
            }

            idx0 = end;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fli_sbsrsc;

    #[test]
    fn test_decode_fli_sbsrsc() {
        let src = [
            0x01, 0x00, // skip 1
            0x02, 0x00, // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x23, 0x45, 0x67, 0x89,
            2, (-4i8) as u8,    // skip 2, length -4
            0xAB ];

        let expected = [
            0x00,
            0x00, 0x00, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89,
            0x00, 0x00, 0xAB, 0xAB, 0xAB, 0xAB,
            0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];

        {
            let mut dst = RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
            let res = decode_fli_sbsrsc(&src, &mut dst);
            assert!(res.is_ok());
        }

        assert_eq!(&buf[0..16], &expected[..]);
    }
}
