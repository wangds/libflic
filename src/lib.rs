//! This crate provides routines for encoding and decoding
//! Autodesk Animator FLI and Autodesk Animator Pro FLC files.

extern crate byteorder;
extern crate libc;

pub use errcode::FlicError;
pub use errcode::FlicResult;

/// Raster structure.
#[allow(dead_code)]
pub struct RasterMut<'a> {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    stride: usize,
    buf: &'a mut [u8],
    pal: &'a mut [u8],
}

pub mod errcode;
pub mod ffi;

mod raster;
