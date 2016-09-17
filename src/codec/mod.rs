//! FLIC encoding and decoding subroutines.

macro_rules! module {
    ($e:ident) => {
        pub use self::$e::*;
        mod $e;
    };
}

use ::{FlicError,FlicResult,RasterMut};

module!(codec011);
module!(codec012);
module!(codec013);
module!(codec015);
module!(codec016);

/// Returns true if the chunk type modifies the palette.
pub fn chunk_modifies_palette(magic: u16)
        -> bool {
    magic == FLI_COLOR64
}

/// Decode a chunk, based on the chunk type.
pub fn decode_chunk(magic: u16, buf: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    match magic {
        FLI_COLOR64 => try!(decode_fli_color64(&buf, dst)),
        FLI_LC => try!(decode_fli_lc(&buf, dst)),
        FLI_BLACK => decode_fli_black(dst),
        FLI_BRUN => try!(decode_fli_brun(&buf, dst)),
        FLI_COPY => try!(decode_fli_copy(&buf, dst)),
        _ => return Err(FlicError::BadMagic),
    }

    Ok(())
}