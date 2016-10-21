//! FLIC implementation.

use std::cmp::min;
use std::fs::File;
use std::io::{Cursor,Read,Seek,SeekFrom,Write};
use std::path::{Path,PathBuf};
use byteorder::LittleEndian as LE;
use byteorder::{ReadBytesExt,WriteBytesExt};

use ::{FlicError,FlicResult,Raster,RasterMut};
use ::pstamp::{PostageStamp,write_pstamp_data};
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

/// Magic for a FLC file - Animator Pro FLC Files.
///
/// This is the main animation file format created by Animator Pro.
/// The file contains a 128-byte header, followed by an optional
/// prefix chunk, followed by one or more frame chunks.
///
/// The prefix chunk, if present, contains Animator Pro settings
/// information, CEL placement information, and other auxiliary data.
///
/// A frame chunk exists for each frame in the animation.  In
/// addition, a ring frame follows all the animation frames.  Each
/// frame chunk contains color palette information and/or pixel data.
///
/// The ring frame contains delta-compressed information to loop from
/// the last frame of the FLIC back to the first.  It can be helpful
/// to think of the ring frame as a copy of the first frame,
/// compressed in a different way.  All FLIC files will contain a ring
/// frame, including a single-frame FLIC.
///
/// A FLC file begins with a 128-byte header, described below.  All
/// lengths and offsets are in bytes.  All values stored in the header
/// fields are unsigned.
///
///   Offset | Length |   Name   | Description
///   ------:| ------:|:--------:| -----------------------------------
///        0 |      4 |   size   | The size of the entire animation file, including this file header.
///        4 |      2 |   magic  | File format identifier.  Always 0xAF12.
///        6 |      2 |  frames  | Number of frames in the FLIC.  This count does not include the ring frame.  FLC files have a maximum length of 4000 frames.
///        8 |      2 |   width  | Screen width in pixels.
///       10 |      2 |   height | Screen height in pixels.
///       12 |      2 |   depth  | Bits per pixel (always 8).
///       14 |      2 |   flags  | Set to 0x0003 after ring frame is written and FLIC header is updated.  This indicates that the file was properly finished and closed.
///       16 |      4 |   speed  | Number of milliseconds to delay between each frame during playback.
///       20 |      2 | reserved | Unused word, set to 0.
///       22 |      4 |  created | The MSDOS-formatted date and time of the file's creation.
///       26 |      4 |  creator | The serial number of the Animator Pro program used to create the file.  If the file was created by some other program using the FlicLib development kit, this value is 0x464C4942 ("FLIB").
///       30 |      4 |  updated | The MSDOS-formatted date and time of the file's most recent update.
///       34 |      4 |  updater | Indicates who last updated the file.  See the description of creator.
///       38 |      2 |  aspectx | The x-axis aspect ratio at which the file was created.
///       40 |      2 |  aspecty | The y-axis aspect ratio at which the file was created.  Most often, the x:y aspect ratio will be 1:1.  A 320x200 FLIC has a ratio of 6:5.
///       42 |     38 | reserved | Unused space, set to zeroes.
///       80 |      4 |  oframe1 | Offset from the beginning of the file to the first animation frame chunk.
///       84 |      4 |  oframe2 | Offset from the beginning of the file to the second animation frame chunk.  This value is used when looping from the ring frame back to the second frame during playback.
///       88 |     40 | reserved | Unused space, set to zeroes.
pub const FLIHR_MAGIC: u16 = 0xAF12;

/// Default updater for files written by LibFLIC, "FLRS".
pub const LIBFLIC_UPDATER_ID: u32 = 0x464C5253;

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

/// FLIC animation writer, with a File handle.
///
/// Opens and holds onto the file handle until it is closed.
#[allow(dead_code)]
pub struct FlicFileWriter {
    hdr: FlicHeader,
    offset_frame1: u64,
    offset_frame2: u64,

    filename: PathBuf,
    file: Option<File>,
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
    magic: u16,
    size: u32,
    frame_count: u16,
    w: u16,
    h: u16,
    speed_msec: u32,
    speed_jiffies: u16,
    created: u32,
    creator: u32,
    updated: u32,
    updater: u32,
    aspect_x: u16,
    aspect_y: u16,
}


/// Magic for a FLIC pre-frame chunk - FLIC Prefix Chunk.
///
/// An optional prefix chunk may immediately follow the animation file
/// header.  This chunk is used to store auxiliary data which is not
/// directly involved in the animation playback.  The prefix chunk
/// starts with a 16-byte header (identical in structure to a frame
/// header), as follows:
///
///   Offset | Length |   Name   | Description
///   ------:| ------:|:--------:| -----------------------------------
///        0 |      4 |   size   | The size of the prefix chunk, including this header and all subordinate chunks that follow.
///        4 |      2 |   type   | Prefix chunk identifier.  Always 0xF100.
///        6 |      2 |  chunks  | Number of subordinate chunks in the prefix chunk.
///        8 |      8 | reserved | Unused space, set to zeroes.
///
/// To determine whether a prefix chunk is present, read the 16-byte
/// header following the file header.  If the type value is 0xF100,
/// it's a prefix chunk.  If the value is 0xF1FA it's the first frame
/// chunk, and no prefix chunk exists.
///
/// # Note
///
/// Programs other than Animator Pro should never need to create FLIC
/// files that contain a prefix chunk.  Programs reading a FLIC file
/// should skip the prefix chunk by using the size value in the prefix
/// header to read and discard the prefix, or by seeking directly to
/// the first frame using the oframe1 field from the file header.
pub const FCID_PREFIX: u16 = 0xF100;

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
/// This implies that no change is made to the screen or color
/// palette, but the appropriate delay is still inserted during
/// playback.
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
    /// ```no_run
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

    /// Number of milliseconds to delay between each frame during playback.
    pub fn speed_msec(&self) -> u32 {
        self.hdr.speed_msec
    }

    /// Number of jiffies to delay between each frame during playback.
    /// A jiffy is 1/70 of a second.
    pub fn speed_jiffies(&self) -> u16 {
        self.hdr.speed_jiffies
    }

    /// Get the FLIC creator.
    pub fn creator(&self) -> u32 {
        self.hdr.creator
    }

    /// Get the FLIC creation time.
    pub fn creation_time(&self) -> u32 {
        self.hdr.created
    }

    /// Get the most recent updater.
    pub fn updater(&self) -> u32 {
        self.hdr.updater
    }

    /// Get the most recent update time.
    pub fn update_time(&self) -> u32 {
        self.hdr.updated
    }

    /// Get the x-axis aspect ratio.
    pub fn aspect_x(&self) -> u16 {
        self.hdr.aspect_x
    }

    /// Get the y-axis aspect ratio.
    pub fn aspect_y(&self) -> u16 {
        self.hdr.aspect_y
    }

    /// Decode the postage stamp.
    pub fn read_postage_stamp<'a>(&mut self, dst: &'a mut RasterMut<'a>)
            -> FlicResult<()> {
        let mut pstamp = PostageStamp::new(
                self.hdr.w as usize, self.hdr.h as usize, dst);

        for chunk in self.frame_hdr[0].chunks.iter() {
            try!(self.file.seek(SeekFrom::Start(chunk.offset)));

            let mut buf = vec![0; chunk.size as usize];
            try!(self.file.read_exact(&mut buf));

            let done = try!(pstamp.feed(chunk.magic, &buf));
            if done {
                break;
            }
        }

        Ok(())
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
    /// ```no_run
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

impl FlicFileWriter {
    /// Open a file for writing Animator Pro FLCs.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// const SCREEN_W: u16 = 320;
    /// const SCREEN_H: u16 = 200;
    /// const speed_msec: u32 = 70;
    ///
    /// flic::FlicFileWriter::create(Path::new("ex.flc"), SCREEN_W, SCREEN_H, speed_msec);
    /// ```
    pub fn create(filename: &Path, w: u16, h: u16, speed_msec: u32)
            -> FlicResult<Self> {
        let mut file = try!(File::create(filename));

        // Reserve space for header.
        try!(file.write_all(&[0; SIZE_OF_FLIC_HEADER]));

        let jiffy_speed = min((speed_msec as u64) * 70 / 1000, ::std::u16::MAX as u64) as u16;

        let hdr = FlicHeader {
            magic: FLIHR_MAGIC,
            size: 0,
            frame_count: 0,
            w: w,
            h: h,
            speed_msec: speed_msec,
            speed_jiffies: jiffy_speed,
            created: 0,
            creator: 0,
            updated: 0,
            updater: LIBFLIC_UPDATER_ID,
            aspect_x: 1,
            aspect_y: 1,
        };

        Ok(FlicFileWriter{
            hdr: hdr,
            offset_frame1: 0,
            offset_frame2: 0,
            filename: filename.to_path_buf(),
            file: Some(file),
        })
    }

    /// Open a file for writing Animator FLIs.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// const speed_jiffies: u16 = 5;
    ///
    /// flic::FlicFileWriter::create_fli(Path::new("ex.fli"), speed_jiffies);
    /// ```
    pub fn create_fli(filename: &Path, speed_jiffies: u16)
            -> FlicResult<Self> {
        let mut file = try!(File::create(filename));

        // Reserve space for header.
        try!(file.write_all(&[0; SIZE_OF_FLIC_HEADER]));

        let hdr = FlicHeader {
            magic: FLIH_MAGIC,
            size: 0,
            frame_count: 0,
            w: 320,
            h: 200,
            speed_msec: (speed_jiffies as u32) * 1000 / 70,
            speed_jiffies: speed_jiffies,
            created: 0,
            creator: 0,
            updated: 0,
            updater: LIBFLIC_UPDATER_ID,
            aspect_x: 6,
            aspect_y: 5,
        };

        Ok(FlicFileWriter{
            hdr: hdr,
            offset_frame1: 0,
            offset_frame2: 0,
            filename: filename.to_path_buf(),
            file: Some(file),
        })
    }

    /// Set the FLIC creator and creation time.
    pub fn set_creator(&mut self, creator: u32, created: u32) {
        self.hdr.creator = creator;
        self.hdr.created = created;
    }

    /// Set the most recent updater and update time.
    pub fn set_updater(&mut self, updater: u32, updated: u32) {
        self.hdr.updater = updater;
        self.hdr.updated = updated;
    }

    /// Set the aspect ratio, i.e. x by y is a square.
    ///
    /// Most often, the x:y aspect ratio will be 1:1.
    /// A 320x200 FLIC has a ratio of 6:5.
    pub fn set_aspect_ratio(&mut self, x: u16, y: u16) {
        if x > 0 && y > 0 {
            self.hdr.aspect_x = x;
            self.hdr.aspect_y = y;
        } else {
            self.hdr.aspect_x = 1;
            self.hdr.aspect_y = 1;
        }
    }

    /// Close the FLIC file.
    ///
    /// You must close the FLIC writer after you have supplied all the
    /// frames, including the ring frame, to write out the header.
    ///
    /// The FLIC writer is not usable after being closed.
    pub fn close(mut self)
            -> FlicResult<()> {
        if let Some(mut file) = self.file.take() {
            if self.hdr.frame_count == 0 {
                return Err(FlicError::Corrupted);
            } else if self.hdr.frame_count == 1 {
                self.offset_frame2 = try!(file.seek(SeekFrom::Current(0)));
                try!(write_empty_frame(&mut file));
            } else {
                self.hdr.frame_count = self.hdr.frame_count - 1;
            }

            let size = try!(file.seek(SeekFrom::Current(0)));
            if size > ::std::u32::MAX as u64 {
                return Err(FlicError::ExceededLimit);
            }

            self.hdr.size = size as u32;
            try!(file.seek(SeekFrom::Start(0)));
            try!(write_flic_header(
                    &self.hdr, self.offset_frame1, self.offset_frame2,
                    &mut file));

            Ok(())
        } else {
            Err(FlicError::NoFile)
        }
    }

    /// Encode the next frame in the FLIC.
    ///
    /// You must supply the previous frame buffer, or None if it is
    /// the first frame.  Upon reaching the last frame in the
    /// animation, you must also supply the first frame to create the
    /// ring frame.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// const SCREEN_W: u16 = 320;
    /// const SCREEN_H: u16 = 200;
    /// const NUM_COLS: usize = 256;
    /// const speed_msec: u32 = 70;
    /// let buf = [0; (SCREEN_W * SCREEN_H) as usize];
    /// let pal = [0; 3 * NUM_COLS];
    ///
    /// if let Ok(mut flic) = flic::FlicFileWriter::create(
    ///         Path::new("ex.flc"), SCREEN_W, SCREEN_H, speed_msec) {
    ///     let raster1 = flic::Raster::new(SCREEN_W as usize, SCREEN_H as usize, &buf, &pal);
    ///     let raster2 = flic::Raster::new(SCREEN_W as usize, SCREEN_H as usize, &buf, &pal);
    ///     // Write first frame.
    ///     flic.write_next_frame(None, &raster1);
    ///     // Write subsequent frames.
    ///     flic.write_next_frame(Some(&raster1), &raster2);
    ///     // Write ring frame.
    ///     flic.write_next_frame(Some(&raster2), &raster1);
    ///     flic.close();
    /// }
    /// ```
    pub fn write_next_frame(&mut self, prev: Option<&Raster>, next: &Raster)
            -> FlicResult<()> {
        if let Some(mut file) = self.file.as_ref() {
            if (next.w != self.hdr.w as usize) || (next.h != self.hdr.h as usize) {
                return Err(FlicError::WrongResolution);
            }
            if self.hdr.frame_count == ::std::u16::MAX {
                return Err(FlicError::ExceededLimit);
            }

            if self.hdr.frame_count == 0 {
                self.offset_frame1 = try!(file.seek(SeekFrom::Current(0)));
            } else if self.hdr.frame_count == 1 {
                self.offset_frame2 = try!(file.seek(SeekFrom::Current(0)));
            }

            let prev = if self.hdr.frame_count == 0 {
                None
            } else {
                prev
            };

            try!(write_next_frame(self.hdr.magic, self.hdr.frame_count,
                    prev, next, &mut file));
            self.hdr.frame_count = self.hdr.frame_count + 1;

            Ok(())
        } else {
            Err(FlicError::NoFile)
        }
    }
}

impl Drop for FlicFileWriter {
    /// A method called when the value goes out of scope.
    fn drop(&mut self) {
        if self.file.is_some() {
            println!("Warning: {} was not closed, may be corrupt.",
                    self.filename.to_string_lossy());
        }
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

    match magic {
        FLIH_MAGIC => read_fli_header(&mut r, size, magic),
        FLIHR_MAGIC => read_flc_header(&mut r, size, magic),
        _ => Err(FlicError::BadMagic),
    }
}

/// Read the original Animator FLI header.
fn read_fli_header(
        r: &mut Cursor<&[u8]>, size: u32, magic: u16)
        -> FlicResult<FlicHeader> {
    assert_eq!(magic, FLIH_MAGIC);

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
        magic: magic,
        size: size,
        frame_count: frame_count,
        w: width,
        h: height,
        speed_msec: (jiffy_speed as u32) * 1000 / 70,
        speed_jiffies: jiffy_speed,
        created: 0,
        creator: 0,
        updated: 0,
        updater: 0,
        aspect_x: 6,
        aspect_y: 5,
    })
}

/// Read the Animator Pro FLC header.
fn read_flc_header(
        r: &mut Cursor<&[u8]>, size: u32, magic: u16)
        -> FlicResult<FlicHeader> {
    assert_eq!(magic, FLIHR_MAGIC);

    let frame_count = try!(r.read_u16::<LE>());
    let width = try!(r.read_u16::<LE>());
    let height = try!(r.read_u16::<LE>());
    let _bpp = try!(r.read_u16::<LE>());
    let _flags = try!(r.read_u16::<LE>());
    let speed = try!(r.read_u32::<LE>());
    try!(r.seek(SeekFrom::Current(2)));
    let created = try!(r.read_u32::<LE>());
    let creator = try!(r.read_u32::<LE>());
    let updated = try!(r.read_u32::<LE>());
    let updater = try!(r.read_u32::<LE>());
    let mut aspect_x = try!(r.read_u16::<LE>());
    let mut aspect_y = try!(r.read_u16::<LE>());
    try!(r.seek(SeekFrom::Current(38)));
    let _oframe1 = try!(r.read_u32::<LE>());
    let _oframe2 = try!(r.read_u32::<LE>());

    match r.seek(SeekFrom::Current(40)) {
        Ok(128) => (),
        _ => unreachable!(),
    };

    if frame_count <= 0 || width <= 0 || height <= 0 {
        return Err(FlicError::Corrupted);
    }

    let jiffy_speed = min((speed as u64) * 70 / 1000, ::std::u16::MAX as u64) as u16;

    if aspect_x <= 0 || aspect_y <= 0 {
        aspect_x = 1;
        aspect_y = 1;
    }

    Ok(FlicHeader{
        magic: magic,
        size: size,
        frame_count: frame_count,
        w: width,
        h: height,
        speed_msec: speed,
        speed_jiffies: jiffy_speed,
        created: created,
        creator: creator,
        updated: updated,
        updater: updater,
        aspect_x: aspect_x,
        aspect_y: aspect_y,
    })
}

/// Read all of the FLIC's frame headers.
fn read_frame_headers(file: &mut File, hdr: &FlicHeader)
        -> FlicResult<Vec<FlicFrame>> {
    let mut offset = SIZE_OF_FLIC_HEADER as u64;
    let mut frames = Vec::new();

    // Add 1 to frame count to account for the ring frame.
    for frame_num in 0..(hdr.frame_count + 1) {
        let mut buf = [0; SIZE_OF_FLIC_FRAME];
        let mut size;
        let mut magic;
        let mut num_chunks;

        try!(file.seek(SeekFrom::Start(offset)));
        try!(file.read_exact(&mut buf));

        {
            let mut r = Cursor::new(&buf[..]);
            size = try!(r.read_u32::<LE>());
            magic = try!(r.read_u16::<LE>());
            num_chunks = try!(r.read_u16::<LE>()) as usize;

            if size < (SIZE_OF_FLIC_FRAME as u32)
                    || offset + (size as u64) > (hdr.size as u64) {
                return Err(FlicError::Corrupted);
            }
        }

        if frame_num == 0 && magic == FCID_PREFIX {
            offset = offset + size as u64;

            try!(file.seek(SeekFrom::Start(offset)));
            try!(file.read_exact(&mut buf));

            let mut r = Cursor::new(&buf[..]);
            size = try!(r.read_u32::<LE>());
            magic = try!(r.read_u16::<LE>());
            num_chunks = try!(r.read_u16::<LE>()) as usize;

            if size < (SIZE_OF_FLIC_FRAME as u32)
                    || offset + (size as u64) > (hdr.size as u64) {
                return Err(FlicError::Corrupted);
            }
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

            FLI_COLOR256 | FLI_SS2 | FLI_COLOR64 | FLI_LC | FLI_BLACK | FLI_BRUN | FLI_PSTAMP => (),

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

/*--------------------------------------------------------------*/

/// Write the FLIC header.
fn write_flic_header<W: Write + Seek>(
        hdr: &FlicHeader, offset_frame1: u64, offset_frame2: u64, w: &mut W)
        -> FlicResult<()> {
    match hdr.magic {
        FLIH_MAGIC => write_fli_header(hdr, w),
        FLIHR_MAGIC => write_flc_header(hdr, offset_frame1, offset_frame2, w),
        _ => return Err(FlicError::BadMagic),
    }
}

fn write_fli_header<W: Write + Seek>(
        hdr: &FlicHeader, w: &mut W)
        -> FlicResult<()> {
    let depth = 8;
    let flags = 0;
    try!(w.write_u32::<LE>(hdr.size));
    try!(w.write_u16::<LE>(FLIH_MAGIC));
    try!(w.write_u16::<LE>(hdr.frame_count));
    try!(w.write_u16::<LE>(hdr.w));
    try!(w.write_u16::<LE>(hdr.h));
    try!(w.write_u16::<LE>(depth));
    try!(w.write_u16::<LE>(flags));
    try!(w.write_u16::<LE>(hdr.speed_jiffies));
    Ok(())
}

fn write_flc_header<W: Write + Seek>(
        hdr: &FlicHeader, offset_frame1: u64, offset_frame2: u64, w: &mut W)
        -> FlicResult<()> {
    let depth = 8;
    let flags = 3;

    try!(w.write_u32::<LE>(hdr.size));
    try!(w.write_u16::<LE>(FLIHR_MAGIC));
    try!(w.write_u16::<LE>(hdr.frame_count));
    try!(w.write_u16::<LE>(hdr.w));
    try!(w.write_u16::<LE>(hdr.h));
    try!(w.write_u16::<LE>(depth));
    try!(w.write_u16::<LE>(flags));
    try!(w.write_u32::<LE>(hdr.speed_msec));

    try!(w.seek(SeekFrom::Current(2))); // reserved
    try!(w.write_u32::<LE>(hdr.created));
    try!(w.write_u32::<LE>(hdr.creator));
    try!(w.write_u32::<LE>(hdr.updated));
    try!(w.write_u32::<LE>(hdr.updater));
    try!(w.write_u16::<LE>(hdr.aspect_x));
    try!(w.write_u16::<LE>(hdr.aspect_y));
    try!(w.seek(SeekFrom::Current(38)));

    // If the offsets are too big, then leave them as 0 and hope other
    // libraries will compute it themselves.
    if offset_frame1 < offset_frame2 && offset_frame2 <= ::std::u32::MAX as u64 {
        try!(w.write_u32::<LE>(offset_frame1 as u32));
        try!(w.write_u32::<LE>(offset_frame2 as u32));
    }

    Ok(())
}

/// Write an empty frame.
fn write_empty_frame<W: Write>(
        w: &mut W)
        -> FlicResult<()> {
    try!(w.write_u32::<LE>(SIZE_OF_FLIC_FRAME as u32));
    try!(w.write_u16::<LE>(FCID_FRAME));
    try!(w.write_u16::<LE>(0)); // chunks
    try!(w.write_all(&[0; 8]));
    Ok(())
}

/// Write the next frame.
fn write_next_frame<W: Write + Seek>(
        flic_magic: u16, frame_count: u16,
        prev: Option<&Raster>, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let pos0 = try!(w.seek(SeekFrom::Current(0)));

    // Reserve space for chunk.
    try!(w.write_all(&[0; SIZE_OF_FLIC_FRAME]));

    let size_pstamp =
        if flic_magic != FLIH_MAGIC && frame_count == 0 {
            match write_pstamp_data(next, w) {
                Ok(size) => size,
                Err(_) => {
                    try!(w.seek(SeekFrom::Start(pos0 + SIZE_OF_FLIC_FRAME as u64)));
                    0
                },
            }
        } else {
            0
        };

    let size_col = try!(write_color_data(flic_magic, prev, next, w));
    let size_pix = try!(write_pixel_data(flic_magic, prev, next, w));
    let size = SIZE_OF_FLIC_FRAME + size_pstamp + size_col + size_pix;

    if size > ::std::u32::MAX as usize {
        return Err(FlicError::ExceededLimit);
    }

    let pos1 = try!(w.seek(SeekFrom::Current(0)));

    try!(w.seek(SeekFrom::Start(pos0)));
    if size > 0 {
        let num_chunks
            = if size_pstamp > 0 { 1 } else { 0 }
            + if size_col > 0 { 1 } else { 0 }
            + if size_pix > 0 { 1 } else { 0 };

        assert_eq!(size, (pos1 - pos0) as usize);
        try!(w.write_u32::<LE>(size as u32));
        try!(w.write_u16::<LE>(FCID_FRAME));
        try!(w.write_u16::<LE>(num_chunks));
        try!(w.seek(SeekFrom::Start(pos1)));
        Ok(size)
    } else {
        Ok(0)
    }
}

/// Write the next frame's palette.
fn write_color_data<W: Write + Seek>(
        flic_magic: u16, prev: Option<&Raster>, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let pos0 = try!(w.seek(SeekFrom::Current(0)));

    // Reserve space for chunk.
    try!(w.write_all(&[0; SIZE_OF_CHUNK]));

    let (chunk_size, chunk_magic) =
        if flic_magic == FLIH_MAGIC {
            let size = try!(encode_fli_color64(prev, next, w));
            (size, FLI_COLOR64)
        } else {
            let size = try!(encode_fli_color256(prev, next, w));
            (size, FLI_COLOR256)
        };

    if SIZE_OF_CHUNK + chunk_size > ::std::u32::MAX as usize {
        return Err(FlicError::ExceededLimit);
    }

    let pos1 = try!(w.seek(SeekFrom::Current(0)));

    try!(w.seek(SeekFrom::Start(pos0)));
    if chunk_size > 0 {
        try!(w.write_u32::<LE>((SIZE_OF_CHUNK + chunk_size) as u32));
        try!(w.write_u16::<LE>(chunk_magic));
        try!(w.seek(SeekFrom::Start(pos1)));
        Ok(SIZE_OF_CHUNK + chunk_size)
    } else {
        Ok(0)
    }
}

/// Write the next frame's pixels.
fn write_pixel_data<W: Write + Seek>(
        flic_magic: u16, prev: Option<&Raster>, next: &Raster, w: &mut W)
        -> FlicResult<usize> {
    let pos0 = try!(w.seek(SeekFrom::Current(0)));

    // Reserve space for chunk.
    try!(w.write_all(&[0; SIZE_OF_CHUNK]));

    let mut chunk_size = next.w * next.h;
    let mut chunk_magic = FLI_COPY;

    // Try FLI_BLACK for first frame only.
    if chunk_magic == FLI_COPY && prev.is_none() {
        if can_encode_fli_black(next) {
            chunk_size = 0;
            chunk_magic = FLI_BLACK;
        }
    }

    // Try FLI_LC.
    if chunk_magic == FLI_COPY && prev.is_some() {
        match encode_fli_lc(prev.unwrap(), next, w) {
            Ok(size) =>
                if size == 0 {
                    try!(w.seek(SeekFrom::Start(pos0)));
                    return Ok(0);
                } else if size < chunk_size {
                    chunk_size = size;
                    chunk_magic = FLI_LC;
                },

            Err(FlicError::ExceededLimit) => (),
            Err(e) => return Err(e),
        }

        if chunk_magic != FLI_LC {
            try!(w.seek(SeekFrom::Start(pos0 + SIZE_OF_CHUNK as u64)));
        }
    }

    // Try FLI_SS2, which has higher limits.
    if flic_magic == FLIHR_MAGIC && chunk_magic == FLI_COPY && prev.is_some() {
        match encode_fli_ss2(prev.unwrap(), next, w) {
            Ok(size) =>
                if size < chunk_size {
                    chunk_size = size;
                    chunk_magic = FLI_SS2;
                },

            Err(FlicError::ExceededLimit) => {},
            Err(e) => return Err(e),
        }

        if chunk_magic != FLI_SS2 {
            try!(w.seek(SeekFrom::Start(pos0 + SIZE_OF_CHUNK as u64)));
        }
    }

    // Try FLI_BRUN.
    if chunk_magic == FLI_COPY {
        match encode_fli_brun(next, w) {
            Ok(size) =>
                if size < chunk_size {
                    chunk_size = size;
                    chunk_magic = FLI_BRUN;
                },

            Err(FlicError::ExceededLimit) => (),
            Err(e) => return Err(e),
        }

        if chunk_magic != FLI_BRUN {
            try!(w.seek(SeekFrom::Start(pos0 + SIZE_OF_CHUNK as u64)));
        }
    }

    // Try FLI_COPY.
    if chunk_magic == FLI_COPY {
        chunk_size = try!(encode_fli_copy(next, w));
        chunk_magic = FLI_COPY;
    }

    let pos1 = try!(w.seek(SeekFrom::Current(0)));
    assert_eq!(SIZE_OF_CHUNK + chunk_size, (pos1 - pos0) as usize);

    try!(w.seek(SeekFrom::Start(pos0)));
    if pos1 - pos0 > ::std::u32::MAX as u64 {
        return Err(FlicError::ExceededLimit);
    }

    try!(w.write_u32::<LE>((pos1 - pos0) as u32));
    try!(w.write_u16::<LE>(chunk_magic));
    try!(w.seek(SeekFrom::Start(pos1)));

    Ok((pos1 - pos0) as usize)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor,Seek,SeekFrom};
    use byteorder::LittleEndian as LE;
    use byteorder::ReadBytesExt;
    use ::Raster;
    use ::codec::FLI_COPY;
    use super::{FLIH_MAGIC,SIZE_OF_CHUNK,write_pixel_data};

    /// Test write_pixel_data output when reverting to FLI_COPY.
    #[test]
    fn test_write_pixel_data_fli_copy() {
        const SCREEN_W: usize = 1;
        const SCREEN_H: usize = 1;
        const NUM_COLS: usize = 256;
        let expected_size = SIZE_OF_CHUNK + SCREEN_W * SCREEN_H;

        let buf = [0xFF; SCREEN_W * SCREEN_H];
        let pal = [0; 3 * NUM_COLS];
        let next = Raster::new(SCREEN_W, SCREEN_H, &buf, &pal);
        let mut w = Cursor::new(Vec::new());

        let res = write_pixel_data(FLIH_MAGIC, None, &next, &mut w);
        assert_eq!(res.expect("size"), expected_size);

        w.seek(SeekFrom::Start(0)).expect("reset");
        assert_eq!(w.read_u32::<LE>().expect("size"), expected_size as u32);
        assert_eq!(w.read_u16::<LE>().expect("magic"), FLI_COPY);
    }
}
