//! Codec for chunk type 16 = FLI_COPY.

use ::{FlicError,FlicResult,RasterMut};

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
