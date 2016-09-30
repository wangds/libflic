//! Foreign function interface.

use std::ffi::CStr;
use std::io::Cursor;
use std::path::Path;
use std::mem;
use std::ptr;
use std::slice;
use libc::{c_char,c_uint,size_t};

use ::{Raster,RasterMut};
use codec::*;
use flic::FlicFile;

/// Dummy opaque structure, equivalent to Raster<'a>.
pub struct CRaster;

/// Dummy opaque structure, equivalent to RasterMut<'a>.
pub struct CRasterMut;

// Print with "file:line - " prefix, for more informative error messages.
macro_rules! printerrorln {
    ($e:expr) => {
        println!("{}:{} - {}", file!(), line!(), $e);
    };
    ($fmt:expr, $arg:tt) => {
        print!("{}:{} - ", file!(), line!());
        println!($fmt, $arg);
    };
}

// Typical common decoder function signature:
//  decode(src: &[u8], dst: &mut RasterMut) -> FlicError<...>
macro_rules! run_decoder {
    ($func:ident($src:ident, $src_len:ident, $dst:ident)) => {{
        let src_slice = unsafe{ slice::from_raw_parts($src, $src_len) };
        let dst_raster = unsafe{ transmute_raster_mut($dst) };
        $func(src_slice, dst_raster)
    }}
}

unsafe fn transmute_raster<'a>(src: *const CRaster)
        -> &'a Raster<'a> {
    let ptr: *const Raster = mem::transmute(src);
    &*ptr
}

unsafe fn transmute_raster_mut<'a>(dst: *mut CRasterMut)
        -> &'a mut RasterMut<'a> {
    let ptr: *mut RasterMut = mem::transmute(dst);
    &mut *ptr
}

/*--------------------------------------------------------------*/
/* Codecs                                                       */
/*--------------------------------------------------------------*/

/// Decode a FLI_WRUN chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_wrun(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut) {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return;
    }

    match run_decoder!(decode_fli_wrun(src, src_len, dst)) {
        Ok(_) => return,
        Err(e) => printerrorln!(e),
    }
}

/// Decode a FLI_SBSRSC chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_sbsrsc(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut) {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return;
    }

    match run_decoder![decode_fli_sbsrsc(src, src_len, dst)] {
        Ok(_) => return,
        Err(e) => printerrorln!(e),
    }
}

/// Decode a FLI_COLOR64 chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_color64(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    match run_decoder![decode_fli_color64(src, src_len, dst)] {
        Ok(_) => return 0,
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Decode a FLI_LC chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_lc(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    match run_decoder![decode_fli_lc(src, src_len, dst)] {
        Ok(_) => return 0,
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Decode a FLI_BLACK chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_black(
        dst: *mut CRasterMut) {
    if dst.is_null() {
        printerrorln!("bad input parameters");
        return;
    }

    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    decode_fli_black(dst_raster);
}

/// Decode a FLI_ICOLORS chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_icolors(
        dst: *mut CRasterMut) {
    if dst.is_null() {
        printerrorln!("bad input parameters");
        return;
    }

    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    decode_fli_icolors(dst_raster);
}

/// Decode a FLI_BRUN chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_brun(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    match run_decoder![decode_fli_brun(src, src_len, dst)] {
        Ok(_) => return 0,
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Decode a FLI_COPY chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_copy(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    match run_decoder![decode_fli_copy(src, src_len, dst)] {
        Ok(_) => return 0,
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Encode a FLI_COLOR64 chunk.
#[no_mangle]
pub extern "C" fn flicrs_encode_fli_color64(
        opt_prev: *const CRaster, next: *const CRaster,
        out_buf: *mut u8, max_len: size_t, out_len: *mut size_t)
        -> c_uint {
    if next.is_null() || out_buf.is_null() || out_len.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let prev_raster = if opt_prev.is_null() {
        None
    } else {
        unsafe{ Some(transmute_raster(opt_prev)) }
    };
    let next_raster = unsafe{ transmute_raster(next) };

    let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match encode_fli_color64(prev_raster, next_raster, &mut enc) {
        Ok(len) => {
            unsafe{ ptr::write(out_len, len) };
            if len <= max_len {
                let dst_slice = unsafe{ slice::from_raw_parts_mut(out_buf, max_len) };
                dst_slice[0..len].copy_from_slice(&enc.get_ref()[..]);
                return 0;
            } else {
                printerrorln!("output buffer too small");
                return 2;
            }
        },
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Encode a FLI_LC chunk.
#[no_mangle]
pub extern "C" fn flicrs_encode_fli_lc(
        prev: *const CRaster, next: *const CRaster,
        out_buf: *mut u8, max_len: size_t, out_len: *mut size_t)
        -> c_uint {
    if prev.is_null() || next.is_null() || out_buf.is_null() || out_len.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let prev_raster = unsafe{ transmute_raster(prev) };
    let next_raster = unsafe{ transmute_raster(next) };

    let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match encode_fli_lc(prev_raster, next_raster, &mut enc) {
        Ok(len) => {
            unsafe{ ptr::write(out_len, len) };
            if len <= max_len {
                assert_eq!(len, enc.get_ref().len());
                let dst_slice = unsafe{ slice::from_raw_parts_mut(out_buf, max_len) };
                dst_slice[0..len].copy_from_slice(&enc.get_ref()[..]);
                return 0;
            } else {
                printerrorln!("output buffer too small");
                return 2;
            }
        },
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Encode a FLI_BRUN chunk.
#[no_mangle]
pub extern "C" fn flicrs_encode_fli_brun(
        next: *const CRaster,
        out_buf: *mut u8, max_len: size_t, out_len: *mut size_t)
        -> c_uint {
    if next.is_null() || out_buf.is_null() || out_len.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let next_raster = unsafe{ transmute_raster(next) };

    let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match encode_fli_brun(next_raster, &mut enc) {
        Ok(len) => {
            unsafe{ ptr::write(out_len, len) };
            if len <= max_len {
                let dst_slice = unsafe{ slice::from_raw_parts_mut(out_buf, max_len) };
                dst_slice[0..len].copy_from_slice(&enc.get_ref()[..]);
                return 0;
            } else {
                printerrorln!("output buffer too small");
                return 2;
            }
        },
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/// Encode a FLI_COPY chunk.
#[no_mangle]
pub extern "C" fn flicrs_encode_fli_copy(
        next: *const CRaster,
        out_buf: *mut u8, max_len: size_t, out_len: *mut size_t)
        -> c_uint {
    if next.is_null() || out_buf.is_null() || out_len.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let next_raster = unsafe{ transmute_raster(next) };

    let mut enc: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match encode_fli_copy(next_raster, &mut enc) {
        Ok(len) => {
            unsafe{ ptr::write(out_len, len) };
            if len <= max_len {
                let dst_slice = unsafe{ slice::from_raw_parts_mut(out_buf, max_len) };
                dst_slice[0..len].copy_from_slice(&enc.get_ref()[..]);
                return 0;
            } else {
                printerrorln!("output buffer too small");
                return 2;
            }
        },
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/*--------------------------------------------------------------*/
/* FLIC                                                         */
/*--------------------------------------------------------------*/

/// Open a FLIC file.
#[no_mangle]
pub extern "C" fn flicrs_open(filename: *const c_char)
        -> *mut FlicFile {
    if filename.is_null() {
        printerrorln!("bad input parameters");
        return ptr::null_mut();
    }

    let cstr = unsafe{ CStr::from_ptr(filename) };
    match cstr.to_str() {
        Ok(s) => match FlicFile::open(Path::new(s)) {
            Ok(f) => {
                return Box::into_raw(Box::new(f));
            },
            Err(e) => {
                printerrorln!(e);
                return ptr::null_mut()
            }
        },
        Err(e) => {
            printerrorln!(e);
            return ptr::null_mut();
        }
    }
}

/// Close a FLIC file.
#[no_mangle]
pub extern "C" fn flicrs_close(flic: *mut FlicFile) {
    if flic.is_null() {
        return;
    }

    let _flic = unsafe{ Box::from_raw(flic) };
}

/// Get the next frame number.
#[no_mangle]
pub extern "C" fn flicrs_frame(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.frame() as c_uint
}

/// Get the frame count, not including the ring frame.
#[no_mangle]
pub extern "C" fn flicrs_frame_count(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.frame_count() as c_uint
}

/// Get the FLIC width.
#[no_mangle]
pub extern "C" fn flicrs_width(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.width() as c_uint
}

/// Get the FLIC height.
#[no_mangle]
pub extern "C" fn flicrs_height(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.height() as c_uint
}

/// Number of jiffies to delay between each frame during playback.
#[no_mangle]
pub extern "C" fn flicrs_speed_jiffies(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.speed_jiffies() as c_uint
}

/// Decode the next frame in the FLIC.
#[no_mangle]
pub extern "C" fn flicrs_read_next_frame(
        flic: *mut FlicFile, dst: *mut CRasterMut)
        -> c_uint {
    if flic.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let flic = unsafe{ &mut *flic };
    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    match flic.read_next_frame(dst_raster) {
        Ok(r) => {
            return 0
                + (if r.ended { 2 } else { 0 })
                + (if r.looped { 4 } else { 0 })
                + (if r.palette_updated { 8 } else { 0 });
        },
        Err(e) => {
            printerrorln!(e);
            return 1;
        },
    }
}

/*--------------------------------------------------------------*/
/* Raster                                                       */
/*--------------------------------------------------------------*/

/// Allocate a new raster.
#[no_mangle]
pub extern "C" fn flicrs_raster_alloc(
        x: size_t, y: size_t, w: size_t, h: size_t, stride: size_t,
        buf: *const u8, buf_len: size_t,
        pal: *const u8, pal_len: size_t)
        -> *mut CRaster {
    if buf.is_null() || pal.is_null() {
        printerrorln!("bad input parameters");
        return ptr::null_mut();
    }

    let buf_slice = unsafe{ slice::from_raw_parts(buf, buf_len) };
    let pal_slice = unsafe{ slice::from_raw_parts(pal, pal_len) };
    let raster = Raster::with_offset(x, y, w, h, stride, buf_slice, pal_slice);
    let rptr = Box::into_raw(Box::new(raster));
    let cptr: *mut CRaster = unsafe{ mem::transmute(rptr) };
    cptr
}

/// Allocate a new raster.
#[no_mangle]
pub extern "C" fn flicrs_raster_mut_alloc(
        x: size_t, y: size_t, w: size_t, h: size_t, stride: size_t,
        buf: *mut u8, buf_len: size_t,
        pal: *mut u8, pal_len: size_t)
        -> *mut CRasterMut {
    if buf.is_null() || pal.is_null() {
        printerrorln!("bad input parameters");
        return ptr::null_mut();
    }

    let buf_slice = unsafe{ slice::from_raw_parts_mut(buf, buf_len) };
    let pal_slice = unsafe{ slice::from_raw_parts_mut(pal, pal_len) };
    let raster = RasterMut::with_offset(x, y, w, h, stride, buf_slice, pal_slice);
    let rptr = Box::into_raw(Box::new(raster));
    let cptr: *mut CRasterMut = unsafe{ mem::transmute(rptr) };
    cptr
}

/// Free a previously allocated raster.
#[no_mangle]
pub extern "C" fn flicrs_raster_free(raster: *mut CRaster) {
    if raster.is_null() {
        return;
    }

    let rptr: *mut Raster = unsafe{ mem::transmute(raster) };
    let _raster = unsafe{ Box::from_raw(rptr) };
}

/// Free a previously allocated raster.
#[no_mangle]
pub extern "C" fn flicrs_raster_mut_free(raster: *mut CRasterMut) {
    if raster.is_null() {
        return;
    }

    let rptr: *mut RasterMut = unsafe{ mem::transmute(raster) };
    let _raster = unsafe{ Box::from_raw(rptr) };
}
