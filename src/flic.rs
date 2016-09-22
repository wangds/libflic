//! FLIC implementation.

use std::fs::File;
use std::io::{Cursor,Read,Seek,SeekFrom};
use std::path::{Path,PathBuf};
use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;

use ::{FlicError,FlicResult,RasterMut};
use codec::*;

/// Magic for a FLI file - Original Animator FLI Files.
///
/// This animation file format is limited to 320x200 resolution.  It
/// is the main animation file format of the original Animator, and is
/// still used by Animator Pro for creating 320x200 animations.  The
/// file structure is very similar to that of a FLC file.  A FLI file
/// does not contain a prefix chunk, and does not use FLI_PSTAMP or
/// FLI_SS2 data encoding in the frame chunks.
///
/// The file header for a FLI file is a subset of the FLC file header.
/// It is defined as follows:
///
///   Offset | Length |   Name   | Description
///   ------:| ------:|:--------:| -----------------------------------
///        0 |      4 |   size   | The size of the entire animation file, including this file header.
///        4 |      2 |   magic  | File format identifier.  Always 0xAF11.
///        6 |      2 |  frames  | Number of frames in the FLIC.  This count does not include the ring frame.  FLI files have a maximum length of 4000 frames.
///        8 |      2 |   width  | Screen width in pixels.  This is always 320 in a FLI file.
///       10 |      2 |  height  | Screen height in pixels.  This is always 200 in a FLI file.
///       12 |      2 |   depth  | Bits per pixel (always 8).
///       14 |      2 |   flags  | Always zero in a FLI file.
///       16 |      2 |   speed  | Number of jiffies to delay between each frame during playback.  A jiffy is 1/70 of a second.
///       18 |    110 | reserved | Unused space, set to zeroes.
pub const FLIH_MAGIC: u16 = 0xAF11;


/// FLIC animation, with a File handle.
///
/// Opens and holds onto the file handle until it is dropped.
#[allow(dead_code)]
pub struct FlicFile {
    hdr: FlicHeader,
    frame_hdr: Vec<FlicFrame>,
    frame: usize,

    filename: PathBuf,
    file: File,
}


/// Size of a FLIC file header on disk.
///
/// A FLIC file begins with a 128-byte header, described below.  All
/// lengths and offsets are in bytes.  All values stored in the header
/// fields are unsigned.
pub const SIZE_OF_FLIC_HEADER: usize = 128;

/// FLIC header.
#[allow(dead_code)]
struct FlicHeader {
    size: u32,
    frame_count: u16,
    w: u16,
    h: u16,
    speed_jiffies: u16,
}


/// Magic for a FLIC frame - FLIC Frame Chunks.
///
/// Frame chunks contain the pixel and color data for the animation.
/// A frame chunk may contain multiple subordinate chunks, each
/// containing a different type of data for the current frame.  Each
/// frame chunk starts with a 16-byte header that describes the
/// contents of the frame:
///
///   Offset | Length |   Name   | Description
///   ------:| ------:|:--------:| -----------------------------------
///        0 |      4 |   size   | The size of the frame chunk, including this header and all subordinate chunks that follow.
///        4 |      2 |   type   | Frame chunk identifier.
///        6 |      2 |  chunks  | Number of subordinate chunks in the frame chunk.
///        8 |      8 | reserved | Unused space, set to zeroes.
pub const FCID_FRAME: u16 = 0xF1FA; // also: FLIF_MAGIC.

/// Size of a FLIC frame header on disk.
pub const SIZE_OF_FLIC_FRAME: usize = 16;

/// FLIC frame header.
#[allow(dead_code)]
struct FlicFrame {
    chunks: Vec<ChunkId>,
}


/// Size of a chunk header on disk.
///
/// Immediately following the frame header are the frame's subordinate
/// data chunks.  When the chunks count in the frame header is zero,
/// it indicates that this frame is identical to the previous frame.
/// This implies that nochange is made to the screen or color palette,
/// but the appropriate delay is still inserted during playback.
///
/// Each data chunk within a frame chunk is formatted as follows:
///
///   Offset | Length | Name | Description
///   ------:| ------:|:----:| ---------------------------------------
///        0 |      4 | size | The size of the chunk, including this header.
///        4 |      2 | type | Data type identifier.
///        6 | size-6 | data | The color or pixel data.
///
/// The type values in the chunk headers indicate what type of
/// graphics data the chunk contains and which compression method was
/// used to encode the data.
pub const SIZE_OF_CHUNK: usize = 6;

/// Chunk header.
#[allow(dead_code)]
struct ChunkId {
    // Note: offset to the data.
    offset: u64,

    // Note: number of bytes in the data, excluding the chunk header.
    size: u32,

    magic: u16,
}


/// Record containing playback information.
pub struct FlicPlaybackResult {
    pub ended: bool,
    pub looped: bool,
    pub palette_updated: bool,
}

/*--------------------------------------------------------------*/

impl FlicFile {
    /// Open a FLIC file.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// flic::FlicFile::open(Path::new("ex.fli"));
    /// ```
    pub fn open(filename: &Path)
            -> FlicResult<Self> {
        if !filename.exists() {
            return Err(FlicError::NoFile);
        } else if !filename.is_file() {
            return Err(FlicError::NotARegularFile);
        }

        let mut file = try!(File::open(filename));

        let hdr = try!(read_flic_header(&mut file));
        let frame_hdr = try!(read_frame_headers(&mut file, &hdr));

        Ok(FlicFile {
            hdr: hdr,
            frame_hdr: frame_hdr,
            frame: 0,

            filename: filename.to_path_buf(),
            file: file,
        })
    }

    /// Get the next frame number.
    pub fn frame(&self) -> u16 {
        self.frame as u16
    }

    /// Get the frame count, not including the ring frame.
    pub fn frame_count(&self) -> u16 {
        self.hdr.frame_count
    }

    /// Get the FLIC width.
    pub fn width(&self) -> u16 {
        self.hdr.w
    }

    /// Get the FLIC height.
    pub fn height(&self) -> u16 {
        self.hdr.h
    }

    /// Number of jiffies to delay between each frame during playback.
    /// A jiffy is 1/70 of a second.
    pub fn speed_jiffies(&self) -> u16 {
        self.hdr.speed_jiffies
    }

    /// Decode the next frame in the FLIC.
    ///
    /// The raster buffer must contain the previous frame.
    /// The FLIC file will loop when it reaches the last frame.
    ///
    /// Returns a record indicating what was processed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// if let Ok(ref mut flic) = flic::FlicFile::open(Path::new("ex.fli")) {
    ///     const SCREEN_W: usize = 320;
    ///     const SCREEN_H: usize = 200;
    ///     const NUM_COLS: usize = 256;
    ///     let mut buf = [0; SCREEN_W * SCREEN_H];
    ///     let mut pal = [0; 3 * NUM_COLS];
    ///     let mut raster = flic::RasterMut::new(SCREEN_W, SCREEN_H, &mut buf, &mut pal);
    ///
    ///     let res = flic.read_next_frame(&mut raster);
    /// }
    /// ```
    pub fn read_next_frame(&mut self, dst: &mut RasterMut)
            -> FlicResult<FlicPlaybackResult> {
        let mut res = FlicPlaybackResult {
            ended: false,
            looped: false,
            palette_updated: false,
        };

        if (self.hdr.w as usize != dst.w) || (self.hdr.h as usize != dst.h) {
            return Err(FlicError::WrongResolution);
        }

        let frame = &self.frame_hdr[self.frame];
        for chunk in frame.chunks.iter() {
            try!(self.file.seek(SeekFrom::Start(chunk.offset)));

            let mut buf = vec![0; chunk.size as usize];
            try!(self.file.read_exact(&mut buf));

            try!(decode_chunk(chunk.magic, &buf, dst));

            res.palette_updated = res.palette_updated
                    || chunk_modifies_palette(chunk.magic);
        }

        if self.frame + 1 >= self.frame_hdr.len() {
            // Skip to second frame, since FLIC animations include a ring frame.
            self.frame = 1;
            res.looped = true;
        } else {
            self.frame = self.frame + 1;
        }

        if self.frame + 1 >= self.frame_hdr.len() {
            res.ended = true;
        }

        Ok(res)
    }
}

/*--------------------------------------------------------------*/

/// Read the FLIC's header.
fn read_flic_header(file: &mut File)
        -> FlicResult<FlicHeader> {
    let mut buf = [0; SIZE_OF_FLIC_HEADER];
    try!(file.read_exact(&mut buf));

    let mut r = Cursor::new(&buf[..]);
    let size = try!(r.read_u32::<LE>());
    let magic = try!(r.read_u16::<LE>());

    if magic != FLIH_MAGIC {
        return Err(FlicError::BadMagic);
    }

    let frame_count = try!(r.read_u16::<LE>());
    let width = try!(r.read_u16::<LE>());
    let height = try!(r.read_u16::<LE>());
    let _bpp = try!(r.read_u16::<LE>());
    let _flags = try!(r.read_u16::<LE>());
    let jiffy_speed = try!(r.read_u16::<LE>());

    match r.seek(SeekFrom::Current(110)) {
        Ok(128) => (),
        _ => unreachable!(),
    };

    // Animator 1's FLIC files are always 320x200.
    if width != 320 || height != 200 {
        return Err(FlicError::WrongResolution);
    }
    if frame_count <= 0 {
        return Err(FlicError::Corrupted);
    }

    Ok(FlicHeader{
        size: size,
        frame_count: frame_count,
        w: width,
        h: height,
        speed_jiffies: jiffy_speed,
    })
}

/// Read all of the FLIC's frame headers.
fn read_frame_headers(file: &mut File, hdr: &FlicHeader)
        -> FlicResult<Vec<FlicFrame>> {
    let mut offset = SIZE_OF_FLIC_HEADER as u64;
    let mut frames = Vec::new();

    // Add 1 to frame count to account for the ring frame.
    for frame_num in 0..(hdr.frame_count + 1) {
        try!(file.seek(SeekFrom::Start(offset)));

        let mut buf = [0; SIZE_OF_FLIC_FRAME];
        try!(file.read_exact(&mut buf));

        let mut r = Cursor::new(&buf[..]);
        let size = try!(r.read_u32::<LE>());
        let magic = try!(r.read_u16::<LE>());
        let num_chunks = try!(r.read_u16::<LE>()) as usize;

        if size < (SIZE_OF_FLIC_FRAME as u32)
                || offset + (size as u64) > (hdr.size as u64) {
            return Err(FlicError::Corrupted);
        }
        if magic != FCID_FRAME {
            return Err(FlicError::BadMagic);
        }

        let chunks = try!(read_chunk_headers(file, hdr,
                frame_num, offset, size, num_chunks));
        assert_eq!(chunks.len(), num_chunks);

        // Note: Animator forces chunk sizes to be even.  However,
        // Animator 1 did not update the frame header size
        // accordingly.  This resulted in lost data.
        if num_chunks > 0 {
            let position = chunks[num_chunks - 1].offset + chunks[num_chunks - 1].size as u64;
            let expected = offset + size as u64;
            if position > expected {
                println!("Warning: frame {} reads too much - current offset={}, expected offset={}",
                         frame_num, position, expected);
            } else if position < expected {
                println!("Warning: frame {} reads too little - current offset={}, expected offset={}",
                         frame_num, position, expected);
            }
        }

        frames.push(FlicFrame{
            chunks: chunks,
        });

        offset = offset + size as u64;
    }

    Ok(frames)
}

/// Read all of the frame's chunk headers.
fn read_chunk_headers(file: &mut File, hdr: &FlicHeader,
        frame_num: u16, frame_offset: u64, frame_size: u32, num_chunks: usize)
        -> FlicResult<Vec<ChunkId>> {
    let mut chunks = Vec::new();
    let mut offset = frame_offset + SIZE_OF_FLIC_FRAME as u64;

    for _ in 0..num_chunks {
        try!(file.seek(SeekFrom::Start(offset)));

        let mut buf = [0; SIZE_OF_CHUNK];
        try!(file.read_exact(&mut buf));

        let mut r = Cursor::new(&buf[..]);
        let size = try!(r.read_u32::<LE>());
        let magic = try!(r.read_u16::<LE>());

        if !(SIZE_OF_CHUNK as u32 <= size && size <= frame_size) {
            return Err(FlicError::Corrupted);
        }

        let mut size2 = size;

        match magic {
            // Warn about legacy chunk types.
            FLI_WRUN =>
                println!("Warning: frame {} - FLI_WRUN chunk type detected",
                        frame_num),
            FLI_SBSRSC =>
                println!("Warning: frame {} - FLI_SBSRSC chunk type detected",
                        frame_num),
            FLI_ICOLORS =>
                println!("Warning: frame {} - FLI_ICOLORS chunk type detected",
                        frame_num),

            // A bug in Animator and Animator Pro caused FLI_COPY
            // chunks have size = size of data + 4 (size of pointer)
            // instead of size of data + 6 (size of chunk header).
            // The data was still written to disk; only the chunk's
            // size is incorrect.
            FLI_COPY => {
                if size == hdr.w as u32 * hdr.h as u32 + 4 {
                    size2 = hdr.w as u32 * hdr.h as u32 + 6;
                    println!("Warning: frame {} - FLI_COPY has wrong size",
                            frame_num);
                }
            },

            FLI_COLOR64 | FLI_LC | FLI_BLACK | FLI_BRUN => (),

            _ => println!("Warning: frame {} - unrecognised chunk type {}",
                    frame_num, magic),
        }

        chunks.push(ChunkId {
            offset: offset + SIZE_OF_CHUNK as u64,
            size: size2 - SIZE_OF_CHUNK as u32,
            magic: magic,
        });

        offset = offset + size as u64;
    }

    Ok(chunks)
}
