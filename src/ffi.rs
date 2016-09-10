//! Foreign function interface.

use std::mem;
use std::ptr;
use std::slice;
use libc::size_t;

use ::RasterMut;

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
