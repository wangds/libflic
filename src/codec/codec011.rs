//! Codec for chunk type 11 = FLI_COLOR64.

use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupByEq};

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

    let count = r.read_u16::<LE>()?;
    for _ in 0..count {
        let nskip = r.read_u8()? as usize;
        let ncopy = match r.read_u8()? {
            0 => 256 as usize,
            n => n as usize,
        };

        let start = idx0 + 3 * nskip;
        let end = start + 3 * ncopy;
        if end > dst.pal.len() {
            return Err(FlicError::Corrupted);
        }

        r.read_exact(&mut dst.pal[start..end])?;
        for i in start..end {
            if dst.pal[i] > ::std::u8::MAX / 4 {
                return Err(FlicError::Corrupted);
            }

            dst.pal[i] = 4 * dst.pal[i];
        }

        idx0 = end;
    }

    Ok(())
}

/// Encode a FLI_COLOR64 chunk.
pub fn encode_fli_color64<W: Write + Seek>(
        prev: Option<&Raster>, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    match prev {
        Some(prev) => encode_fli_color64_delta(prev, next, w),
        None => encode_fli_color64_full(next, w),
    }
}

fn encode_fli_color64_full<W: Write + Seek>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if next.pal.len() % 3 != 0 || next.pal.len() > 3 * 256 {
        return Err(FlicError::BadInput);
    }

    let pos0 = w.seek(SeekFrom::Current(0))?;

    let count = 1;
    let nskip = 0;
    let ncopy = (next.pal.len() / 3) as u8;
    w.write_u16::<LE>(count)?;
    w.write_u8(nskip)?;
    w.write_u8(ncopy)?;

    for i in 0..next.pal.len() {
        w.write_u8(next.pal[i] / 4)?;
    }

    let pos1 = w.seek(SeekFrom::Current(0))?;
    Ok((pos1 - pos0) as usize)
}

fn encode_fli_color64_delta<W: Write + Seek>(
        prev: &Raster, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if prev.pal.len() != next.pal.len()
            || prev.pal.len() % 3 != 0
            || next.pal.len() % 3 != 0 {
        return Err(FlicError::BadInput);
    }

    // Reserve space for count.
    let pos0 = w.seek(SeekFrom::Current(0))?;
    w.write_u16::<LE>(0)?;

    let mut count = 0;

    for g in GroupByEq::new(prev.pal.chunks(3), next.pal.chunks(3))
            .set_prepend_same_run()
            .set_ignore_final_same_run() {
        match g {
            Group::Same(_, nskip) => {
                assert!(nskip <= ::std::u8::MAX as usize);
                w.write_u8(nskip as u8)?;
            },
            Group::Diff(idx, ncopy) => {
                let start = 3 * idx;
                let end = start + 3 * ncopy;
                assert!(ncopy <= ::std::u8::MAX as usize + 1);
                w.write_u8(ncopy as u8)?;

                for i in start..end {
                    w.write_u8(next.pal[i] / 4)?;
                }
            },
        }

        count = count + 1;
    }

    // If odd number, pad it to be even.
    let mut pos1 = w.seek(SeekFrom::Current(0))?;
    if (pos1 - pos0) % 2 == 1 {
        w.write_u8(0)?;
        pos1 = pos1 + 1;
    }

    w.seek(SeekFrom::Start(pos0))?;
    if count > 0 {
        assert!(count % 2 == 0);
        assert!(count / 2 <= ::std::u16::MAX as u32);
        w.write_u16::<LE>((count / 2) as u16)?;
        w.seek(SeekFrom::Start(pos1))?;

        Ok((pos1 - pos0) as usize)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use ::{Raster,RasterMut};
    use super::*;

    #[test]
    fn test_decode_fli_color64() {
        let src = [
            0x02, 0x00, // count 2
            1, 2,       // skip 1, copy 2
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            3, 4,       // skip 3, copy 4
            0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F ];

        let expected = [
            0x00, 0x00, 0x00,
            4*0x0A, 4*0x0B, 4*0x0C, 4*0x1A, 4*0x1B, 4*0x1C,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            4*0x2A, 4*0x2B, 4*0x2C, 4*0x2D, 4*0x2E, 4*0x2F, 4*0x3A, 4*0x3B, 4*0x3C, 4*0x3D, 4*0x3E, 4*0x3F,
            0x00, 0x00, 0x00 ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fli_color64(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&pal[0..33], &expected[..]);
    }

    #[test]
    fn test_encode_fli_color64_delta() {
        let src = [
            4*0x0A, 4*0x0B, 4*0x0C, 4*0x1A, 4*0x1B, 4*0x1C,
            4*0x00, 4*0x00, 4*0x00, 4*0x00, 4*0x00, 4*0x00, 4*0x00, 4*0x00, 4*0x00,
            4*0x2A, 4*0x2B, 4*0x2C, 4*0x3A, 4*0x3B, 4*0x3C, 4*0x3D, 4*0x3E, 4*0x3F,
            4*0x00, 4*0x00, 4*0x00 ];

        let expected = [
            0x02, 0x00, // count 2
            0, 2,       // skip 0, copy 2
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            3, 3,       // skip 3, copy 3
            0x2A, 0x2B, 0x2C, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
            0x00 ];     // even

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let buf = [0; SCREEN_W * SCREEN_H];
        let pal1 = [0; 3 * NUM_COLS];
        let mut pal2 = [0; 3 * NUM_COLS];
        pal2[0..27].copy_from_slice(&src[..]);

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let prev = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal1);
        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal2);
        let res = encode_fli_color64(Some(&prev), &next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }

    #[test]
    fn test_encode_fli_color64_full() {
        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;

        let buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];
        let mut expected = [0; 4 + 3 * NUM_COLS];
        expected[0] = 0x01; // count 1, skip 0, copy 256

        for i in 0..NUM_COLS {
            let c = (i as u8) / 4;

            pal[3 * i + 0] = 4 * c;
            pal[3 * i + 1] = 4 * c;
            pal[3 * i + 2] = 4 * c;

            expected[4 + 3 * i + 0] = c;
            expected[4 + 3 * i + 1] = c;
            expected[4 + 3 * i + 2] = c;
        }

        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
        let res = encode_fli_color64(None, &next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }
}
