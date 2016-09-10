//! Foreign function interface.

use std::mem;
use std::ptr;
use std::slice;
use libc::{c_uint,size_t};

use ::RasterMut;
use codec::*;

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

unsafe fn transmute_raster_mut<'a>(dst: *mut CRasterMut)
        -> &'a mut RasterMut<'a> {
    let ptr: *mut RasterMut = mem::transmute(dst);
    &mut *ptr
}

/*--------------------------------------------------------------*/
/* Codecs                                                       */
/*--------------------------------------------------------------*/

/// Decode a FLI_COLOR64 chunk.
#[no_mangle]
pub extern "C" fn flicrs_decode_fli_color64(
        src: *const u8, src_len: usize, dst: *mut CRasterMut)
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
        src: *const u8, src_len: usize, dst: *mut CRasterMut)
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

/*--------------------------------------------------------------*/
/* Raster                                                       */
/*--------------------------------------------------------------*/

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
pub extern "C" fn flicrs_raster_mut_free(raster: *mut CRasterMut) {
    if raster.is_null() {
        return;
    }

    let rptr: *mut RasterMut = unsafe{ mem::transmute(raster) };
    let _raster = unsafe{ Box::from_raw(rptr) };
}
