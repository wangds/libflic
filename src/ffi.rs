//! Foreign function interface.

use std::ffi::CStr;
use std::io::Cursor;
use std::path::Path;
use std::mem;
use std::ptr;
use std::slice;
use libc::{c_char,c_uint,size_t};

use ::{FlicFile,FlicResult,Raster,RasterMut};
use codec::*;

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

fn run_decoder<F>(file: &'static str, line: u32,
        decoder: F,
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint
        where F: FnOnce(&[u8], &mut RasterMut) -> FlicResult<()> {
    let src_slice = unsafe{ slice::from_raw_parts(src, src_len) };
    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    match decoder(src_slice, dst_raster) {
        Ok(_) => return 0,
        Err(e) => {
            println!("{}:{} - {}", file, line, e);
            return 1;
        },
    }
}

fn run_encoder<F>(file: &'static str, line: u32,
        encoder: F,
        out_buf: *mut u8, max_len: size_t, out_len: *mut size_t)
        -> c_uint
        where F: FnOnce(&mut Cursor<Vec<u8>>) -> FlicResult<usize> {
    let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match encoder(&mut buf) {
        Ok(len) => {
            unsafe{ ptr::write(out_len, len) };
            if len <= max_len {
                assert_eq!(len, buf.get_ref().len());
                let dst_slice = unsafe{ slice::from_raw_parts_mut(out_buf, max_len) };
                dst_slice[0..len].copy_from_slice(&buf.get_ref()[..]);
                return 0;
            } else {
                println!("{}:{} - output buffer too small", file, line);
                return 2;
            }
        },
        Err(e) => {
            println!("{}:{} - {}", file, line, e);
            return 1;
        },
    }
}

/*--------------------------------------------------------------*/
/* Codecs                                                       */
/*--------------------------------------------------------------*/

/// Decode a FLI_WRUN chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_wrun(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_wrun(src, dst),
            src, src_len, dst)
}

/// Decode a FLI_COLOR256 chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_color256(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_color256(src, dst),
            src, src_len, dst)
}

/// Decode a FLI_SS2 chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_ss2(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_ss2(src, dst),
            src, src_len, dst)
}

/// Decode a FLI_SBSRSC chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_sbsrsc(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_sbsrsc(src, dst),
            src, src_len, dst)
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

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_color64(src, dst),
            src, src_len, dst)
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

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_lc(src, dst),
            src, src_len, dst)
}

/// Decode a FLI_BLACK chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_black(
        dst: *mut CRasterMut)
        -> c_uint {
    if dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    decode_fli_black(dst_raster);
    return 0;
}

/// Decode a FLI_ICOLORS chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_icolors(
        dst: *mut CRasterMut)
        -> c_uint {
    if dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    decode_fli_icolors(dst_raster);
    return 0;
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

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_brun(src, dst),
            src, src_len, dst)
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

    run_decoder(file!(), line!(),
            |src, dst| decode_fli_copy(src, dst),
            src, src_len, dst)
}

/// Decode a FPS_BRUN chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fps_brun(
        src: *const u8, src_len: size_t, src_w: size_t, src_h: size_t,
        dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || src_w <= 0 || src_h <= 0 || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fps_brun(src, src_w, src_h, dst),
            src, src_len, dst)
}

/// Decode a FPS_COPY chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fps_copy(
        src: *const u8, src_len: size_t, src_w: size_t, src_h: size_t,
        dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || src_w <= 0 || src_h <= 0 || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    run_decoder(file!(), line!(),
            |src, dst| decode_fps_copy(src, src_w, src_h, dst),
            src, src_len, dst)
}

/// Create the postage stamp's six-cube palette.
#[no_mangle]
pub extern "C" fn flicrs_make_pstamp_pal(
        dst: *mut CRasterMut)
        -> c_uint {
    if dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    make_pstamp_pal(dst_raster);
    return 0;
}

/// Create a translation table to map the palette into the postage
/// stamp's six-cube palette.
#[no_mangle]
pub extern "C" fn flicrs_make_pstamp_xlat256(
        src: *const u8, src_len: size_t, dst: *mut u8, dst_len: size_t)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let src_slice = unsafe{ slice::from_raw_parts(src, src_len) };
    let dst_slice = unsafe{ slice::from_raw_parts_mut(dst, dst_len) };
    make_pstamp_xlat256(src_slice, dst_slice);
    return 0;
}

/// Apply the translation table to the pixels in the raster.
#[no_mangle]
pub extern "C" fn flicrs_apply_pstamp_xlat256(
        src: *const u8, src_len: size_t, dst: *mut CRasterMut)
        -> c_uint {
    if src.is_null() || dst.is_null() {
        printerrorln!("bad input parameters");
        return 1;
    }

    let src_slice = unsafe{ slice::from_raw_parts(src, src_len) };
    let dst_raster = unsafe{ transmute_raster_mut(dst) };
    apply_pstamp_xlat256(src_slice, dst_raster);
    return 0;
}

/// Encode a FLI_COLOR256 chunk.
#[no_mangle]
pub extern "C" fn flicrs_encode_fli_color256(
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
    run_encoder(file!(), line!(),
            |w| encode_fli_color256(prev_raster, next_raster, w),
            out_buf, max_len, out_len)
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
    run_encoder(file!(), line!(),
            |w| encode_fli_color64(prev_raster, next_raster, w),
            out_buf, max_len, out_len)
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
    run_encoder(file!(), line!(),
            |w| encode_fli_lc(prev_raster, next_raster, w),
            out_buf, max_len, out_len)
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
    run_encoder(file!(), line!(),
            |w| encode_fli_brun(next_raster, w),
            out_buf, max_len, out_len)
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
    run_encoder(file!(), line!(),
            |w| encode_fli_copy(next_raster, w),
            out_buf, max_len, out_len)
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

/// Number of milliseconds to delay between each frame during playback.
#[no_mangle]
pub extern "C" fn flicrs_speed_msec(flic: *const FlicFile)
        -> c_uint {
    if flic.is_null() {
        printerrorln!("bad input parameters");
        return 0;
    }

    let flic = unsafe{ &*flic };
    flic.speed_msec() as c_uint
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
