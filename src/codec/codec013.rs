//! Codec for chunk type 13 = FLI_BLACK.

use ::RasterMut;

/// Magic for a FLI_BLACK chunk - No Data.
///
/// This chunk has no data following the header.  All pixels in the
/// frame are set to color index 0.
pub const FLI_BLACK: u16 = 13;

/// Decode a FLI_BLACK chunk.
pub fn decode_fli_black(dst: &mut RasterMut) {
    let start = dst.stride * dst.y;
    let end = dst.stride * (dst.y + dst.h);
    for row in dst.buf[start..end].chunks_mut(dst.stride) {
        let start = dst.x;
        let end = start + dst.w;
        for e in &mut row[start..end] {
            *e = 0;
        }
    }
}
