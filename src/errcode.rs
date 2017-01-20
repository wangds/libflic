//! FLIC error codes.

use std::io;

pub type FlicResult<T> = Result<T, FlicError>;

quick_error! {

#[derive(Debug)]
pub enum FlicError {
    // Generic failure.  Please try to make something more meaningful.
    NoGood {
        description("No good")
    }

    BadInput {
        description("Bad input")
    }
    NoFile {
        description("File not found")
    }
    NotARegularFile {
        description("Not a regular file")
    }
    BadMagic {
        description("Bad magic")
    }
    Corrupted {
        description("Corrupted")
    }
    WrongResolution {
        description("Wrong resolution")
    }
    ExceededLimit {
        description("Exceeded limit")
    }

    Io(err: io::Error) {
        from()
        description(err.description())
        display("IO error: {}", err)
        cause(err)
    }
}

}
