//! Codec for chunk type 18 = FLI_PSTAMP.

use ::RasterMut;

/// Magic for a FLI_PSTAMP chunk - Postage Stamp Image.
///
/// This chunk type holds a postage stamp -- a reduced-size image --
/// of the frame.  It generally appears only in the first frame chunk
/// within a FLIC file.
///
/// When creating a postage stamp, Animator Pro considers the ideal
/// size to be 100x63 pixels.  The actual size will vary as needed to
/// maintain the same aspect ratio as the original.
///
/// The pixels in a postage stamp image are mapped into a six-cube
/// color space, regardless of the color palette settings for the full
/// frame image.  A six-cube color space is formed as follows:
///
/// ```text
///   start at palette entry 0
///   for red = 0 through 5
///       for green = 0 through 5
///           for blue = 0 through 5
///               palette_red   = (red   * 256) / 6
///               palette_green = (green * 256) / 6
///               palette_blue  = (blue  * 256) / 6
///               move to next palette entry
///           end for blue
///       end for green
///   end for red
/// ```
///
/// Any arbitrary RGB value (where each component is in the range of
/// 0-255) can be mapped into the six-cube space using the formula:
///
/// ```text
///   ((6 * red) / 256) * 36 + ((6 * green) / 256) * 6 + ((6 * blue) / 256)
/// ```
///
/// When a frame data chunk has been identified as a postage stamp,
/// the header for the chunk contains more fields than just size and
/// type.  The full postage stamp chunk header is defined as follows:
///
///   Offset | Length |  Name  | Description
///   ------:| ------:|:------:| -------------------------------------
///        0 |      4 |  size  | The size of the postage stamp chunk, including this header.
///        4 |      2 |  type  | Postage stamp identifier; always 18.
///        6 |      2 | height | Height of the postage stamp image, in pixels.
///        8 |      2 |  width | Width of the postage stamp image, in pixels.
///       10 |      2 |  xlate | Color translation type; always 1, indicating six-cube color space.
///
/// Immediately following this header is the postage stamp data.  The
/// data is formatted as a chunk with standard size/type header.  The
/// type will be one of:
///
///   Value | Name        | Description
///   -----:| ----------- | ------------------------------------------
///      15 | FPS_BRUN    | Byte run length compression.
///      16 | FPS_COPY    | No compression.
///      18 | FPS_XLAT256 | Six-cube color xlate table.
///
/// The FPS_BRUN and FPS_COPY types are identical to the FLI_BRUN and
/// FLI_COPY encoding methods described above.
///
/// The FPS_XLAT256 type indicates that the chunk contains a 256-byte
/// color translation table instead of pixel data.  To process this
/// type of postage stamp, read the pixel data for the full-sized
/// frame image, and translate its pixels into six-cube space using a
/// lookup in the 256-byte color translation table.  This type of
/// postage stamp appears when the size of the animation frames is
/// smaller than the standard 100x63 postage stamp size.
pub const FLI_PSTAMP: u16 = 18;

/// Magic for a FPS_XLAT256 chunk - Postage Stamp, Six-Cube Color Translation Table.
pub const FPS_XLAT256: u16 = FLI_PSTAMP;

/// Create the postage stamp's six-cube palette.
pub fn make_pstamp_pal(dst: &mut RasterMut) {
    assert!(dst.pal.len() >= 6 * 6 * 6);

    let mut c = 0;
    for r in 0..6 {
        for g in 0..6 {
            for b in 0..6 {
                dst.pal[3 * c + 0] = ((r * 256) / 6) as u8;
                dst.pal[3 * c + 1] = ((g * 256) / 6) as u8;
                dst.pal[3 * c + 2] = ((b * 256) / 6) as u8;
                c = c + 1;
            }
        }
    }
}

/// Create a translation table to map the palette into the postage
/// stamp's six-cube palette.
pub fn make_pstamp_xlat256(pal: &[u8], xlat256: &mut [u8]) {
    assert_eq!(pal.len(), 3 * xlat256.len());

    for c in 0..xlat256.len() {
        let r = pal[3 * c + 0] as u32;
        let g = pal[3 * c + 1] as u32;
        let b = pal[3 * c + 2] as u32;

        xlat256[c]
            = (((6 * r) / 256) * 36
            +  ((6 * g) / 256) * 6
            +  ((6 * b) / 256)) as u8;
    }
}

/// Apply the translation table to the pixels in the raster, mapping
/// the pixels to the postage stamp's six-cube palette.
pub fn apply_pstamp_xlat256(xlat256: &[u8], dst: &mut RasterMut) {
    assert!(xlat256.len() >= ::std::u8::MAX as usize);

    let start = dst.stride * dst.y;
    let end = dst.stride * (dst.y + dst.h);
    for row in dst.buf[start..end].chunks_mut(dst.stride) {
        let start = dst.x;
        let end = start + dst.w;
        for e in &mut row[start..end] {
            *e = xlat256[*e as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use ::RasterMut;
    use super::*;

    #[test]
    fn test_make_pstamp_xlat256() {
        let src = [
            0x00, 0x00, 0x00,
            0x00, 0x00, 0xFF,
            0x00, 0xFF, 0x00,
            0x00, 0xFF, 0xFF,
            0xFF, 0x00, 0x00,
            0xFF, 0x00, 0xFF,
            0xFF, 0xFF, 0x00,
            0xFF, 0xFF, 0xFF ];

        let expected = [
            0, 5, 30, 35, 180, 185, 210, 215 ];

        let mut xlat256 = [0; 8];

        make_pstamp_xlat256(&src, &mut xlat256);
        assert_eq!(&xlat256[..], &expected[..]);
    }

    #[test]
    fn test_apply_pstamp_xlat256() {
        let src = [
            0, 5, 30, 35, 180, 185, 210, 215 ];

        let mut buf = [
            7, 6, 5, 4, 3, 2, 1, 0 ];

        let expected = [
            215, 210, 185, 180, 35, 30, 5, 0 ];

        const SCREEN_W: usize = 8;
        const SCREEN_H: usize = 1;
        let mut pal = [0; 3 * 256];

        let mut xlat256 = [0; 256];
        xlat256[0..8].copy_from_slice(&src[..]);

        apply_pstamp_xlat256(&xlat256,
                &mut RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal));
        assert_eq!(&buf[..], &expected[..]);
    }
}
