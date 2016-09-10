//! FLIC encoding and decoding subroutines.

macro_rules! module {
    ($e:ident) => {
        pub use self::$e::*;
        mod $e;
    };
}

module!(codec011);
module!(codec012);
module!(codec013);
module!(codec015);
