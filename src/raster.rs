//! Raster implementation.

use ::{Raster,RasterMut};

impl<'a> Raster<'a> {
    /// Allocate a new raster for the given screen buffer and palette
    /// memory slices.
    ///
    /// # Examples
    ///
    /// ```
    /// const SCREEN_W: usize = 320;
    /// const SCREEN_H: usize = 200;
    /// const NUM_COLS: usize = 256;
    /// let buf = [0; SCREEN_W * SCREEN_H];
    /// let pal = [0; 3 * NUM_COLS];
    ///
    /// flic::Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
    /// ```
    pub fn new(w: usize, h: usize, buf: &'a [u8], pal: &'a [u8])
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
    /// let buf = [0; SCREEN_W * SCREEN_H];
    /// let pal = [0; 3 * NUM_COLS];
    ///
    /// flic::Raster::with_offset(0, 0, SCREEN_W, SCREEN_H, SCREEN_W, &buf, &pal);
    /// ```
    pub fn with_offset(
            x: usize, y: usize, w: usize, h: usize, stride: usize,
            buf: &'a [u8], pal: &'a [u8])
            -> Self {
        let x1 = x.checked_add(w).expect("overflow");
        let y1 = y.checked_add(h).expect("overflow");
        assert!(x < x1 && x1 <= stride && h > 0);
        assert!(stride.checked_mul(y1).expect("overflow") <= buf.len());
        assert!(pal.len() == 3 * 256);

        Raster {
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
        let x1 = x.checked_add(w).expect("overflow");
        let y1 = y.checked_add(h).expect("overflow");
        assert!(x < x1 && x1 <= stride && h > 0);
        assert!(stride.checked_mul(y1).expect("overflow") <= buf.len());
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

#[cfg(test)]
mod tests {
    use ::{Raster,RasterMut};

    #[test]
    #[should_panic]
    fn test_raster_overflow() {
        let buf = [0; 1];
        let pal = [0; 3 * 256];
        let _ = Raster::new(
                ::std::usize::MAX, ::std::usize::MAX, &buf, &pal);
    }

    #[test]
    #[should_panic]
    fn test_raster_mut_overflow() {
        let mut buf = [0; 1];
        let mut pal = [0; 3 * 256];
        let _ = RasterMut::new(
                ::std::usize::MAX, ::std::usize::MAX, &mut buf, &mut pal);
    }
}
