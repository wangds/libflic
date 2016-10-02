//! Codec for chunk type 7 = FLI_SS2.

use std::io::{Cursor,Read};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};

/// Magic for a FLI_SS2 chunk - Word Aligned Delta Compression.
///
/// This format contains the differences between consecutive frames.
/// This is the format most often used by Animator Pro for frames
/// other than the first frame of an animation.  It is similar to the
/// line coded delta (LC) compression, but is word oriented instead of
/// byte oriented.  The data is organized into lines and each line is
/// organized into packets.
///
/// The first word in the data following the chunk header contains the
/// number of lines in the chunk.  Each line can begin with some
/// optional words that are used to skip lines and set the last byte
/// in the line for animations with odd widths.  These optional words
/// are followed by a count of the packets in the line.  The line
/// count does not include skipped lines.
///
/// The high order two bits of the word is used to determine the
/// contents of the word.
///
///   Bit 15 | Bit 14 | Meaning
///  :------:|:------:| ----------------------------------------------
///      0   |    0   | The word contains the packet count.  The packets follow this word.  The packet count can be zero; this occurs when only the last pixel on a line changes.
///      1   |    0   | The low order byte is to be stored in the last byte of the current line.  The packet count always follows this word.
///      1   |    1   | The word contains a line skip count.  The number of lines skipped is given by the absolute value of the word.  This word can be followed by more skip counts, by a last byte word, or by the packet count.
///
/// The packets in each line are similar to the packets for the line
/// coded chunk.  The first byte of each packet is a column skip
/// count.  The second byte is a packet type.  If the packet type is
/// positive, the packet type is a count of words to be copied from
/// the packet to the animation image.  If the packet type is
/// negative, the packet contains one more word which is to be
/// replicated.  The absolute value of the packet type gives the
/// number of times the word is to be replicated.  The high and low
/// order byte in the replicated word do not necessarily have the same
/// value.
pub const FLI_SS2: u16 = 7;

/// Decode a FLI_SS2 chunk.
pub fn decode_fli_ss2(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    let mut r = Cursor::new(src);
    let mut y = 0;

    let mut h = try!(r.read_u16::<LE>()) as usize;
    while y < dst.h && h > 0 {
        let mut count = try!(r.read_u16::<LE>());

        if (count & (1 << 15)) != 0 {
            if (count & (1 << 14)) != 0 {
                // Skip lines.
                y = y + (-((count as i16) as i32)) as usize;
                continue;
            } else {
                // Write last byte.
                let idx = dst.stride * (dst.y + y) + (dst.x + dst.w - 1);
                dst.buf[idx] = count as u8;

                count = try!(r.read_u16::<LE>());
                if count == 0 {
                    y = y + 1;
                    h = h - 1;
                    continue;
                }
            }
        }

        let start = dst.stride * (dst.y + y);
        let end = dst.stride * (dst.y + y + 1);
        let mut row = &mut dst.buf[start..end];
        let mut x0 = dst.x;

        for _ in 0..count {
            let nskip = try!(r.read_u8()) as usize;
            let signed_length = try!(r.read_i8()) as i32;

            if signed_length >= 0 {
                let start = x0 + nskip;
                let end = start + 2 * signed_length as usize;
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                try!(r.read_exact(&mut row[start..end]));

                x0 = end;
            } else {
                let start = x0 + nskip;
                let end = start + 2 * (-signed_length) as usize;
                let c0 = try!(r.read_u8());
                let c1 = try!(r.read_u8());
                if end > row.len() {
                    return Err(FlicError::Corrupted);
                }

                for e in &mut row[start..end].chunks_mut(2) {
                    e[0] = c0;
                    e[1] = c1;
                }

                x0 = end;
            }
        }

        y = y + 1;
        h = h - 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::*;

    #[test]
    fn test_decode_fli_ss2() {
        let src = [
            0x02, 0x00, // hh 2
            0x02, 0x00, // count 2
            3, 5,       // skip 3, length 5
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            2, (-4i8) as u8,    // skip 2, length -4
            0xAB, 0xCD,
            0xFF, 0xFF, // count -1
            0xEE, 0x80, // bit15 = 1, bit14 = 0, data = 0xFF
            0x00, 0x00, // count 0
        ];
        let expected = [
            0x00, 0x00, 0x00,
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90,
            0x00, 0x00,
            0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD,
        ];

        const SCREEN_W: usize = 320;
        const SCREEN_H: usize = 200;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];

        let res = decode_fli_ss2(&src,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[0..23], &expected[..]);
        assert_eq!(buf[(SCREEN_W * 2) + (SCREEN_W - 1)], 0xEE);
    }
}
