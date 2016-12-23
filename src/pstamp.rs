//! FLIC postage stamp implementation.

use std::io::{Cursor,Seek,SeekFrom,Write};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
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
                    decode_fli_color256(&buf, &mut self.dst)?;
                    self.have_palette = true;
                },
            FLI_COLOR64 =>
                if !self.have_xlat256 {
                    decode_fli_color64(&buf, &mut self.dst)?;
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
                decode_fps_brun(&buf, self.flic_w, self.flic_h, &mut self.dst)?;
                self.have_image = true;
            },
            FLI_COPY => {
                decode_fps_copy(&buf, self.flic_w, self.flic_h, &mut self.dst)?;
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

/// Get the postage stamp size.
pub fn get_pstamp_size(
        max_w: u16, max_h: u16, w: u16, h: u16)
        -> (u16, u16) {
    if max_w <= 0 || max_h <= 0 || w <= 0 || h <= 0 {
        return (0, 0);
    }

    let mut scaled_w;
    let mut scaled_h;

    if (w as u32) * (max_h as u32) / (h as u32) > (max_w as u32) {
        scaled_w = max_w;
        scaled_h = ((h as u32) * (max_w as u32) / (w as u32)) as u16;
    } else {
        scaled_w = ((w as u32) * (max_h as u32) / (h as u32)) as u16;
        scaled_h = max_h;
    }

    if scaled_w <= 0 {
        scaled_w = 1;
    }
    if scaled_h <= 0 {
        scaled_h = 1;
    }

    (scaled_w, scaled_h)
}

/// Decode a FLI_PSTAMP chunk.
///
/// Returns true if the postage stamp has been created.
fn decode_fli_pstamp(
        src: &[u8], dst: &mut RasterMut, xlat256: &mut [u8; 256])
        -> FlicResult<bool> {
    let mut r = Cursor::new(src);
    let height = r.read_u16::<LE>()? as usize;
    let width = r.read_u16::<LE>()? as usize;
    let _xlate = r.read_u16::<LE>()?;

    let size = r.read_u32::<LE>()? as usize;
    let magic = r.read_u16::<LE>()?;
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
                decode_fps_brun(&src[start..end], width, height, dst)?;
                return Ok(true);
            } else {
                return Err(FlicError::WrongResolution);
            },
        FPS_COPY =>
            if width > 0 && height > 0 {
                decode_fps_copy(&src[start..end], width, height, dst)?;
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

/// Write the postage stamp chunk.
pub fn write_pstamp_data<W: Write + Seek>(
        next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    const SIZE_OF_CHUNK_ID: usize = 6;
    const SIZE_OF_SUB_CHUNK: usize = SIZE_OF_CHUNK_ID;
    const SIZE_OF_FULL_CHUNK: usize = SIZE_OF_CHUNK_ID + 6 + SIZE_OF_SUB_CHUNK;

    if next.w > ::std::u16::MAX as usize || next.h > ::std::u16::MAX as usize {
        // We can still write a postage stamp for huge images, but
        // get_pstamp_size() is not smart enough right now.
        return Err(FlicError::ExceededLimit);
    }

    let (pstamp_w, pstamp_h) = get_pstamp_size(
            STANDARD_PSTAMP_W, STANDARD_PSTAMP_H, next.w as u16, next.h as u16);

    if pstamp_w <= 0 || pstamp_h <= 0 || can_encode_fli_black(next) {
        return Ok(0);
    }

    let mut chunk_size = ((pstamp_w as u32) * (pstamp_h as u32)) as usize;
    let mut chunk_magic = FPS_COPY;

    // Reserve space for chunk.
    let pos0 = w.seek(SeekFrom::Current(0))?;
    w.write_all(&[0; SIZE_OF_FULL_CHUNK])?;
    let pos1 = w.seek(SeekFrom::Current(0))?;

    let mut xlat256 = [0; 256];
    make_pstamp_xlat256(&next.pal, &mut xlat256);

    // FPS_XLAT256
    if chunk_magic == FPS_COPY && (next.w * next.h < chunk_size as usize) {
        chunk_size = 256;
        chunk_magic = FPS_XLAT256;

        w.write_all(&xlat256[..])?;
    }

    // FPS_BRUN/FPS_COPY.
    if chunk_magic == FPS_COPY {
        let pstamp_buf = prepare_pstamp(
                next, &xlat256, pstamp_w as usize, pstamp_h as usize);
        let pstamp = Raster::new(
                pstamp_w as usize, pstamp_h as usize, &pstamp_buf, &next.pal);

        match encode_fli_brun(&pstamp, w) {
            Ok(size) =>
                if size < chunk_size {
                    chunk_size = size;
                    chunk_magic = FLI_BRUN;
                },

            Err(FlicError::ExceededLimit) => (),
            Err(e) => return Err(e),
        }

        if chunk_magic == FPS_COPY {
            w.seek(SeekFrom::Start(pos1))?;
            chunk_size = encode_fli_copy(&pstamp, w)?;
            chunk_magic = FPS_COPY;
        }
    }

    let pos2 = w.seek(SeekFrom::Current(0))?;
    assert_eq!(SIZE_OF_FULL_CHUNK + chunk_size, (pos2 - pos0) as usize);

    w.seek(SeekFrom::Start(pos0))?;
    if pos2 - pos0 > ::std::u32::MAX as u64 {
        return Err(FlicError::ExceededLimit);
    }

    w.write_u32::<LE>((SIZE_OF_FULL_CHUNK + chunk_size) as u32)?;
    w.write_u16::<LE>(FLI_PSTAMP)?;
    w.write_u16::<LE>(pstamp_h)?;
    w.write_u16::<LE>(pstamp_w)?;
    w.write_u16::<LE>(PSTAMP_SIXCUBE)?;
    w.write_u32::<LE>((SIZE_OF_SUB_CHUNK + chunk_size) as u32)?;
    w.write_u16::<LE>(chunk_magic)?;
    w.seek(SeekFrom::Start(pos2))?;

    Ok((pos2 - pos0) as usize)
}
