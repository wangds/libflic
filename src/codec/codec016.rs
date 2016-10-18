//! Codec for chunk type 16 = FLI_COPY.

use std::io::Write;

use ::{FlicError,FlicResult,Raster,RasterMut};
use super::linscale;

/// Magic for a FLI_COPY chunk - No Compression.
///
/// This chunk contains an uncompressed image of the frame.  The
/// number of pixels following the chunk header is exactly the width
/// of the animation times the height of the animation.  The data
/// starts in the upper left corner with pixels copied from left to
/// right and then top to bottom.  This type of chunk is created when
/// the preferred compression method (SS2 or BRUN) generates more data
/// than the uncompressed frame image; a relatively rare situation.
pub const FLI_COPY: u16 = 16;

/// Magic for a FPS_COPY chunk - Postage Stamp, No Compression.
pub const FPS_COPY: u16 = FLI_COPY;

/// Decode a FLI_COPY chunk.
pub fn decode_fli_copy(src: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    if (dst.w <= 0) || (src.len() % dst.w != 0) {
        return Err(FlicError::WrongResolution);
    }

    let start = dst.stride * dst.y;
    let end = dst.stride * (dst.y + dst.h);
    let src_rows = src.chunks(dst.w);
    let dst_rows = dst.buf[start..end].chunks_mut(dst.stride);
    for (src_row, dst_row) in src_rows.zip(dst_rows) {
        let start = dst.x;
        let end = start + dst.w;
        dst_row[start..end].copy_from_slice(src_row);
    }

    Ok(())
}

/// Decode a FPS_COPY chunk.
pub fn decode_fps_copy(
        src: &[u8], src_w: usize, src_h: usize, dst: &mut RasterMut)
        -> FlicResult<()> {
    if src_w <= 0 || src_h <= 0 || (src_w * src_h > src.len()) {
        return Err(FlicError::WrongResolution);
    }

    for dy in 0..dst.h {
        let sy = linscale(src_h, dst.h, dy);
        let src_start = src_w * sy;
        let src_end = src_start + src_w;
        let dst_start = dst.stride * (dst.y + dy) + dst.x;
        let dst_end = dst_start + dst.w;
        let src_row = &src[src_start..src_end];
        let dst_row = &mut dst.buf[dst_start..dst_end];

        for dx in 0..dst.w {
            let sx = linscale(src_w, dst.w, dx);
            dst_row[dx] = src_row[sx];
        }
    }

    Ok(())
}

/// Encode a FLI_COPY chunk.
pub fn encode_fli_copy<W: Write>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let start = next.stride * next.y;
    let end = next.stride * (next.y + next.h);
    for row in next.buf[start..end].chunks(next.stride) {
        let start = next.x;
        let end = start + next.w;
        try!(w.write_all(&row[start..end]));
    }

    Ok(next.w * next.h)
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::decode_fps_copy;

    #[test]
    fn test_decode_fps_copy() {
        let src = [
            11, 12, 13,
            21, 22, 23,
            31, 32, 33 ];

        let expected = [
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 11, 11, 12, 12, 13, 13,
            0, 0, 11, 11, 12, 12, 13, 13,
            0, 0, 21, 21, 22, 22, 23, 23,
            0, 0, 21, 21, 22, 22, 23, 23,
            0, 0, 31, 31, 32, 32, 33, 33,
            0, 0, 31, 31, 32, 32, 33, 33,
            0, 0, 0, 0, 0, 0, 0, 0 ];

        const SCREEN_W: usize = 8;
        const SCREEN_H: usize = 8;
        const NUM_COLS: usize = 256;
        let mut buf = [0; SCREEN_W * SCREEN_H];
        let mut pal = [0; 3 * NUM_COLS];
        let res = decode_fps_copy(&src, 3, 3,
                &mut RasterMut::with_offset(2, 1, 6, 6, SCREEN_W, &mut buf, &mut pal));
        assert!(res.is_ok());
        assert_eq!(&buf[..], &expected[..]);
    }
}
