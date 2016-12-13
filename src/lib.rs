//! This crate provides routines for encoding and decoding
//! Autodesk Animator FLI and Autodesk Animator Pro FLC files.

extern crate byteorder;
extern crate libc;

pub use errcode::FlicError;
pub use errcode::FlicResult;
pub use flic::FlicFile;
pub use flic::FlicFileWriter;

/// Raster structure.
pub struct Raster<'a> {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    stride: usize,
    buf: &'a [u8],
    pal: &'a [u8],
}

/// Mutable raster structure.
pub struct RasterMut<'a> {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    stride: usize,
    buf: &'a mut [u8],
    pal: &'a mut [u8],
}

pub mod codec;
pub mod ffi;
pub mod flic;
pub mod pstamp;

mod errcode;
mod raster;
