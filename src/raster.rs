//! Raster implementation.

use ::RasterMut;

impl<'a> RasterMut<'a> {
    /// Allocate a new raster for the given screen buffer and palette
    /// memory slices.
    ///
    /// # Examples
    ///
    /// ```
    /// const SCREEN_W: usize = 320;
    /// const SCREEN_H: usize = 200;
    /// const NUM_COLS: usize = 256;
    /// let mut buf = [0; SCREEN_W * SCREEN_H];
    /// let mut pal = [0; 3 * NUM_COLS];
    ///
    /// flic::RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
    /// ```
    pub fn new(w: usize, h: usize, buf: &'a mut [u8], pal: &'a mut [u8])
            -> Self {
        Self::with_offset(0, 0, w, h, w, buf, pal)
    }

    /// Allocate a new raster for the given screen buffer and palette
    /// memory slices, with an offset and stride.
    ///
    /// # Examples
    ///
    /// ```
    /// const SCREEN_W: usize = 320;
    /// const SCREEN_H: usize = 200;
    /// const NUM_COLS: usize = 256;
    /// let mut buf = [0; SCREEN_W * SCREEN_H];
    /// let mut pal = [0; 3 * NUM_COLS];
    ///
    /// flic::RasterMut::with_offset(0, 0, SCREEN_W, SCREEN_H, SCREEN_W, &mut buf, &mut pal);
    /// ```
    pub fn with_offset(
            x: usize, y: usize, w: usize, h: usize, stride: usize,
            buf: &'a mut [u8], pal: &'a mut [u8])
            -> Self {
        assert!(x < stride);
        assert!(x + w <= stride);
        assert!(stride * (y + h) <= buf.len());
        assert!(pal.len() == 3 * 256);

        RasterMut {
            x: x,
            y: y,
            w: w,
            h: h,
            stride: stride,
            buf: buf,
            pal: pal,
        }
    }
}
