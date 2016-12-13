//! FLIC error codes.

use std::error;
use std::fmt;
use std::io;

pub type FlicResult<T> = Result<T, FlicError>;

#[derive(Debug)]
pub enum FlicError {
    // Generic failure.  Please try to make something more meaningful.
    NoGood,

    BadInput,
    NoFile,
    NotARegularFile,
    BadMagic,
    Corrupted,
    WrongResolution,
    ExceededLimit,

    // IO error.
    Io(io::Error),
}

impl fmt::Display for FlicError {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::FlicError::*;
        match *self {
            NoGood => write!(f, "No good"),
            BadInput => write!(f, "Bad input"),
            NoFile => write!(f, "File not found"),
            NotARegularFile => write!(f, "Not a regular file"),
            BadMagic => write!(f, "Bad magic"),
            Corrupted => write!(f, "Corrupted"),
            WrongResolution => write!(f, "Wrong resolution"),
            ExceededLimit => write!(f, "Exceeded limit"),
            Io(ref err) => write!(f, "IO error: {}", err),
        }
    }
}

impl error::Error for FlicError {
    /// A short description of the error.
    fn description(&self) -> &str {
        use self::FlicError::*;
        match *self {
            NoGood => "No good",
            BadInput => "Bad input",
            NoFile => "No file",
            NotARegularFile => "Not a regular file",
            BadMagic => "Bad magic",
            Corrupted => "Corrupted",
            WrongResolution => "Wrong resolution",
            ExceededLimit => "Exceeded limit",
            Io(ref err) => err.description(),
        }
    }

    /// The lower level cause of this error, if any.
    fn cause(&self) -> Option<&error::Error> {
        use self::FlicError::*;
        match *self {
            Io(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for FlicError {
    fn from(err: io::Error) -> FlicError {
        FlicError::Io(err)
    }
}
