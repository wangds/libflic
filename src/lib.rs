//! This crate provides routines for encoding and decoding
//! Autodesk Animator FLI and Autodesk Animator Pro FLC files.

extern crate byteorder;
extern crate libc;

pub use errcode::FlicError;
pub use errcode::FlicResult;

pub mod errcode;
