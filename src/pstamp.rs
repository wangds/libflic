//! FLIC postage stamp implementation.

use std::io::Cursor;
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};
use codec::*;

/// FLIC postage stamp creator.
pub struct PostageStamp<'a> {
    flic_w: usize,
    flic_h: usize,
    have_image: bool,
    have_palette: bool,
    have_xlat256: bool,
    apply_xlat256: bool,
    xlat256: [u8; 256],
    dst: &'a mut RasterMut<'a>,
}

impl<'a> PostageStamp<'a> {
    /// Allocate a new postage stamp creator.
    ///
    /// # Examples
    ///
    /// ```
    /// const SCREEN_W: usize = 320;
    /// const SCREEN_H: usize = 200;
    /// const NUM_COLS: usize = 256;
    /// const PSTAMP_W: usize = 100;
    /// const PSTAMP_H: usize = 63;
    /// let mut buf = [0; PSTAMP_W * PSTAMP_H];
    /// let mut pal = [0; 3 * NUM_COLS];
    /// let mut raster = flic::RasterMut::new(PSTAMP_W, PSTAMP_H, &mut buf, &mut pal);
    ///
    /// flic::pstamp::PostageStamp::new(SCREEN_W, SCREEN_H, &mut raster);
    /// ```
    pub fn new(flic_w: usize, flic_h: usize, dst: &'a mut RasterMut<'a>)
            -> Self {
        assert!(flic_w > 0 && flic_h > 0);

        PostageStamp {
            flic_w: flic_w,
            flic_h: flic_h,
            have_image: false,
            have_palette: false,
            have_xlat256: false,
            apply_xlat256: true,
            xlat256: [0; 256],
            dst: dst,
        }
    }

    /// Feed a chunk (from the first frame) to the postage stamp.
    ///
    /// Returns true if the postage stamp has been created.
    pub fn feed(&mut self, magic: u16, buf: &[u8])
            -> FlicResult<bool> {
        match magic {
            // FLI_WRUN => (),
            // FLI_SS2 => (),
            // FLI_SBSRSC => (),
            FLI_COLOR256 =>
                if !self.have_xlat256 {
                    try!(decode_fli_color256(&buf, &mut self.dst));
                    self.have_palette = true;
                },
            FLI_COLOR64 =>
                if !self.have_xlat256 {
                    try!(decode_fli_color64(&buf, &mut self.dst));
                    self.have_palette = true;
                },
            // FLI_LC => (),
            FLI_BLACK => {
                decode_fli_black(&mut self.dst);
                self.have_image = true;
                self.apply_xlat256 = false;
            },
            FLI_ICOLORS =>
                if !self.have_xlat256 {
                    decode_fli_icolors(&mut self.dst);
                    self.have_palette = true;
                },
            FLI_BRUN => {
                try!(decode_fps_brun(&buf, self.flic_w, self.flic_h, &mut self.dst));
                self.have_image = true;
            },
            FLI_COPY => {
                try!(decode_fps_copy(&buf, self.flic_w, self.flic_h, &mut self.dst));
                self.have_image = true;
            },
            FLI_PSTAMP =>
                match decode_fli_pstamp(&buf, &mut self.dst, &mut self.xlat256) {
                    Ok(true) => {
                        self.have_image = true;
                        self.apply_xlat256 = false;
                    },
                    Ok(false) => {
                        self.have_xlat256 = true;
                    },
                    Err(e) => {
                        // If an error occurred, we can still create
                        // the postage stamp from scratch.
                        println!("Warning: postage stamp - {}", e);
                    },
                },

            _ => return Err(FlicError::BadMagic),
        }

        let done = self.have_image
                && (self.have_palette || self.have_xlat256 || !self.apply_xlat256);
        if done {
            if self.apply_xlat256 {
                if !self.have_xlat256 {
                    make_pstamp_xlat256(&self.dst.pal, &mut self.xlat256);
                }
                apply_pstamp_xlat256(&self.xlat256, &mut self.dst);
            }
            make_pstamp_pal(&mut self.dst);
        }

        Ok(done)
    }
}

/*--------------------------------------------------------------*/

/// Decode a FLI_PSTAMP chunk.
///
/// Returns true if the postage stamp has been created.
fn decode_fli_pstamp(
        src: &[u8], dst: &mut RasterMut, xlat256: &mut [u8; 256])
        -> FlicResult<bool> {
    let mut r = Cursor::new(src);
    let height = try!(r.read_u16::<LE>()) as usize;
    let width = try!(r.read_u16::<LE>()) as usize;
    let _xlate = try!(r.read_u16::<LE>());

    let size = try!(r.read_u32::<LE>()) as usize;
    let magic = try!(r.read_u16::<LE>());
    if size < 6 {
        return Err(FlicError::Corrupted);
    }

    let start = 12;
    let end = start + (size - 6);
    if end > src.len() {
        return Err(FlicError::Corrupted);
    }

    match magic {
        FPS_BRUN =>
            if width > 0 && height > 0 {
                try!(decode_fps_brun(&src[start..end], width, height, dst));
                return Ok(true);
            } else {
                return Err(FlicError::WrongResolution);
            },
        FPS_COPY =>
            if width > 0 && height > 0 {
                try!(decode_fps_copy(&src[start..end], width, height, dst));
                return Ok(true);
            } else {
                return Err(FlicError::WrongResolution);
            },
        FPS_XLAT256 =>
            if size >= 6 + 256 {
                xlat256.copy_from_slice(&src[start..(start + 256)]);
                return Ok(false);
            } else {
                return Err(FlicError::Corrupted);
            },
        _ => return Err(FlicError::BadMagic),
    }
}
