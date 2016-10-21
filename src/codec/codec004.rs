//! Codec for chunk type 4 = FLI_COLOR256.

use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::{Group,GroupByEq};

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

    let count = try!(r.read_u16::<LE>());
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

/// Encode a FLI_COLOR256 chunk.
pub fn encode_fli_color256<W: Write + Seek>(
        prev: Option<&Raster>, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    match prev {
        Some(prev) => encode_fli_color256_delta(prev, next, w),
        None => encode_fli_color256_full(next, w),
    }
}

fn encode_fli_color256_full<W: Write + Seek>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if next.pal.len() % 3 != 0 || next.pal.len() > 3 * 256 {
        return Err(FlicError::BadInput);
    }

    let pos0 = try!(w.seek(SeekFrom::Current(0)));

    let count = 1;
    let nskip = 0;
    let ncopy = (next.pal.len() / 3) as u8;
    try!(w.write_u16::<LE>(count));
    try!(w.write_u8(nskip));
    try!(w.write_u8(ncopy));
    try!(w.write_all(&next.pal[..]));

    let pos1 = try!(w.seek(SeekFrom::Current(0)));
    Ok((pos1 - pos0) as usize)
}

fn encode_fli_color256_delta<W: Write + Seek>(
        prev: &Raster, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    if prev.pal.len() != next.pal.len()
            || prev.pal.len() % 3 != 0
            || next.pal.len() % 3 != 0 {
        return Err(FlicError::BadInput);
    }

    // Reserve space for count.
    let pos0 = try!(w.seek(SeekFrom::Current(0)));
    try!(w.write_u16::<LE>(0));

    let mut count = 0;

    for g in GroupByEq::new(prev.pal.chunks(3), next.pal.chunks(3))
            .set_prepend_same_run()
            .set_ignore_final_same_run() {
        match g {
            Group::Same(_, nskip) => {
                assert!(nskip <= ::std::u8::MAX as usize);
                try!(w.write_u8(nskip as u8));
            },
            Group::Diff(idx, ncopy) => {
                let start = 3 * idx;
                let end = start + 3 * ncopy;
                assert!(ncopy <= ::std::u8::MAX as usize + 1);
                try!(w.write_u8(ncopy as u8));
                try!(w.write_all(&next.pal[start..end]));
            },
        }

        count = count + 1;
    }

    // If odd number, pad it to be even.
    let mut pos1 = try!(w.seek(SeekFrom::Current(0)));
    if (pos1 - pos0) % 2 == 1 {
        try!(w.write_u8(0));
        pos1 = pos1 + 1;
    }

    try!(w.seek(SeekFrom::Start(pos0)));
    if count > 0 {
        assert!(count % 2 == 0);
        assert!(count / 2 <= ::std::u16::MAX as u32);
        try!(w.write_u16::<LE>((count / 2) as u16));
        try!(w.seek(SeekFrom::Start(pos1)));

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
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * 256];

        let res = decode_fli_color256(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&pal[0..33], &expected[..]);
    }

    #[test]
    fn test_encode_fli_color256_delta() {
        let src = [
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x2A, 0x2B, 0x2C, 0x3A, 0x3B, 0x3C, 0x4A, 0x4B, 0x4C,
            0x00, 0x00, 0x00 ];

        let expected = [
            0x02, 0x00, // count 2
            0, 2,       // skip 0, copy 2
            0x0A, 0x0B, 0x0C, 0x1A, 0x1B, 0x1C,
            3, 3,       // skip 3, copy 3
            0x2A, 0x2B, 0x2C, 0x3A, 0x3B, 0x3C, 0x4A, 0x4B, 0x4C,
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
        let res = encode_fli_color256(Some(&prev), &next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[..], &expected[..]);
    }

    #[test]
    fn test_encode_fli_color256_full() {
        let expected = [
            0x01, 0x00, // count 1
            0, 0 ];     // skip 0, copy 256

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let buf = [0; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * NUM_COLS];
        let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
        let res = encode_fli_color256(None, &next, &mut enc);
        assert!(res.is_ok());
        assert_eq!(&enc.get_ref()[0..4], &expected[..]);
        assert_eq!(&enc.get_ref()[4..(4 + 3 * NUM_COLS)], &pal[..]);
    }
}
