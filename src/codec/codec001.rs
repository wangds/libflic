//! Codec for chunk type 1 = FLI_WRUN.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};

/// Magic for a FLI_WRUN chunk.
///
/// This is likely to be used by very old development FLICs only.
pub const FLI_WRUN: u16 = 1;

/// Decode a FLI_WRUN chunk.
///
/// The following logic only makes sense for 320x200 FLICs.
pub fn decode_fli_wrun(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    if dst.x != 0 || dst.y != 0
            || dst.w != 320 || dst.h != 200 || dst.stride != 320 {
        return Err(FlicError::WrongResolution);
    }

    let mut r = Cursor::new(src);
    let mut idx0 = 0;

    let count = try!(r.read_u16::<LE>());
    for _ in 0..count {
        // Read a short, but cast to a signed byte.
        let signed_length = (try!(r.read_u16::<LE>()) as i8) as i32;

        if signed_length >= 0 {
            let start = idx0;
            let end = start + 2 * signed_length as usize;
            if end > dst.buf.len() {
                return Err(FlicError::Corrupted);
            }

            let c0 = try!(r.read_u8());
            let c1 = try!(r.read_u8());
            for e in &mut dst.buf[start..end].chunks_mut(2) {
                e[0] = c0;
                e[1] = c1;
            }

            idx0 = end;
        } else {
            let start = idx0;
            let end = start + 2 * (-signed_length) as usize;
            if end > dst.buf.len() {
                return Err(FlicError::Corrupted);
            }

            try!(r.read_exact(&mut dst.buf[start..end]));

            idx0 = end;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fli_wrun;

    #[test]
    fn test_decode_fli_wrun() {
        let src = [
            0x02, 0x00, // count 2
            0x03, 0xFF, // length = 0xFF03 = +3 (and some garbage)
            0xCD, 0xAB, // data
            0xFC, 0xFF, // length = 0xFFFC = -4 (and some garbage)
            0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0xEF, 0xCD ];

        let expected = [
            0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB,
            0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0xEF, 0xCD,
            0x00, 0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fli_wrun(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[0..16], &expected[..]);
    }
}
