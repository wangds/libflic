//! FLIC encoding and decoding subroutines.

macro_rules! module {
    ($e:ident) => {
        pub use self::$e::*;
        mod $e;
    }
}

use std::iter::Zip;

use ::{FlicError,FlicResult,RasterMut};

module!(codec001);
module!(codec004);
module!(codec007);
module!(codec010);
module!(codec011);
module!(codec012);
module!(codec013);
module!(codec014);
module!(codec015);
module!(codec016);
module!(codec018);

/*--------------------------------------------------------------*/

/// Result of a GroupByX operation: a grouping type, start index, and
/// length.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
enum Group {
    Same(usize, usize),
    Diff(usize, usize),
}

/// An iterator that groups the two input streams based on whether
/// corresponding items are equal.  It returns whether they are equal
/// and the length until that comparison changes value.
///
/// This is suitable for compressing skip/memcpy type codecs,
/// e.g. FLI_COLOR64, FLI_COLOR256.
struct GroupByEq<I: Iterator> where I::Item: PartialEq {
    iter: Zip<I, I>,
    peek: Option<bool>,
    idx: usize,
    prepend_same_run: bool,
    ignore_final_same_run: bool,
}

/// An iterator that groups the two buffers into packets by "runs".
/// A run is a length of the buffer where the corresponding elements
/// of the "old" and "new" buffers are the same, or a stretch of the
/// "new" buffer where all of the elements have the same value.
///
/// This is suitable for compressing skip/memset/memcpy type codes,
/// e.g. FLI_LC, FLI_SS2.
struct GroupByRuns<'a> {
    old: &'a [u8],
    new: &'a [u8],
    idx: usize,
    prepend_same_run: bool,
    ignore_final_same_run: bool,
    group_by_lc: bool,
}

type GroupByLC<'a> = GroupByRuns<'a>;
type GroupBySS2<'a> = GroupByRuns<'a>;

/// An iterator that groups the buffer into packets of the same value.
///
/// This is suitable for compressing memset/memcpy type codecs,
/// e.g. FLI_BRUN.
struct GroupByValue<'a> {
    buf: &'a [u8],
    idx: usize,
}

/// An iterator to help with linear scaling functions.
struct LinScale {
    sw: usize,
    dw: usize,
    sx: usize,
    dx: usize,
    xerr: usize,
}

/*--------------------------------------------------------------*/

/// Returns true if the chunk type modifies the palette.
pub fn chunk_modifies_palette(magic: u16)
        -> bool {
    (magic == FLI_COLOR256) || (magic == FLI_COLOR64) || (magic == FLI_ICOLORS)
}

/// Decode a chunk, based on the chunk type.
pub fn decode_chunk(magic: u16, buf: &[u8], dst: &mut RasterMut)
        -> FlicResult<()> {
    match magic {
        FLI_WRUN => decode_fli_wrun(&buf, dst)?,
        FLI_COLOR256 => decode_fli_color256(&buf, dst)?,
        FLI_SS2 => decode_fli_ss2(&buf, dst)?,
        FLI_SBSRSC => decode_fli_sbsrsc(&buf, dst)?,
        FLI_COLOR64 => decode_fli_color64(&buf, dst)?,
        FLI_LC => decode_fli_lc(&buf, dst)?,
        FLI_BLACK => decode_fli_black(dst),
        FLI_ICOLORS => decode_fli_icolors(dst),
        FLI_BRUN => decode_fli_brun(&buf, dst)?,
        FLI_COPY => decode_fli_copy(&buf, dst)?,

        // Postage stamps should not be decoded in the same loop as
        // the main animation; they have different sizes and work on
        // different buffers and palettes.
        FLI_PSTAMP => (),

        _ => return Err(FlicError::BadMagic),
    }

    Ok(())
}

/*--------------------------------------------------------------*/

impl<I: Iterator> GroupByEq<I>
        where I::Item: PartialEq {
    /// Create a new GroupByEq iterator.
    fn new(old: I, new: I) -> Self {
        GroupByEq {
            iter: old.zip(new),
            peek: None,
            idx: 0,
            prepend_same_run: false,
            ignore_final_same_run: false,
        }
    }

    /// If set, and if the two buffers start on a "Diff" sequence,
    /// then a "Same" group of length 0 will be added at the start.
    fn set_prepend_same_run(mut self) -> Self {
        assert!(self.peek.is_none());
        self.prepend_same_run = true;
        self
    }

    /// If set, and if the two buffers end on a "Same" sequence,
    /// then this final "same" type group will be ignored.
    fn set_ignore_final_same_run(mut self) -> Self {
        self.ignore_final_same_run = true;
        self
    }
}

impl<I: Iterator> Iterator for GroupByEq<I>
        where I::Item: PartialEq {
    type Item = Group;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Group> {
        let expected: bool;
        let start = self.idx;
        let mut n = 0;

        if self.prepend_same_run {
            self.prepend_same_run = false;
            expected = true;
        } else if let Some(x) = self.peek {
            self.peek = None;
            expected = x;
            n = n + 1;
        } else if let Some((a, b)) = self.iter.next() {
            expected = a == b;
            n = n + 1;
        } else {
            return None;
        }

        for x in self.iter.by_ref().map(|(a, b)| a == b) {
            if x == expected {
                n = n + 1;
            } else {
                self.peek = Some(x);
                break;
            }
        }

        self.idx = self.idx + n;
        if expected {
            if self.ignore_final_same_run && self.peek.is_none() {
                return None;
            }
            return Some(Group::Same(start, n));
        } else {
            return Some(Group::Diff(start, n));
        }
    }
}

impl<'a> GroupByRuns<'a> {
    /// Create a new GroupByLC iterator.
    fn new_lc(old: &'a [u8], new: &'a [u8]) -> Self {
        assert_eq!(old.len(), new.len());
        GroupByLC {
            old: old,
            new: new,
            idx: 0,
            prepend_same_run: false,
            ignore_final_same_run: false,
            group_by_lc: true,
        }
    }

    /// Create a new GroupBySS2 iterator.
    fn new_ss2(old: &'a [u8], new: &'a [u8]) -> Self {
        assert_eq!(old.len(), new.len());
        GroupBySS2 {
            old: old,
            new: new,
            idx: 0,
            prepend_same_run: false,
            ignore_final_same_run: false,
            group_by_lc: false,
        }
    }

    /// If set, and if the two buffers start on a "Diff" sequence,
    /// then a "Same" group of length 0 will be added at the start.
    fn set_prepend_same_run(mut self) -> Self {
        self.prepend_same_run = true;
        self
    }

    /// If set, and if the two buffers end on a "Same" sequence,
    /// then this final "same" type group will be ignored.
    fn set_ignore_final_same_run(mut self) -> Self {
        self.ignore_final_same_run = true;
        self
    }
}

impl<'a> Iterator for GroupByRuns<'a> {
    type Item = Group;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Group> {
        let len = self.new.len();
        let start = self.idx;
        let mut i = self.idx;

        if i >= len {
            return None;
        } else if self.old[i] == self.new[i]
                || self.prepend_same_run {
            while (i < len) && (self.old[i] == self.new[i]) {
                i = i + 1;
            }

            let n = i - self.idx;
            self.idx = i;
            self.prepend_same_run = false;

            if i >= len && self.ignore_final_same_run {
                return None;
            } else {
                return Some(Group::Same(start, n));
            }
        }

        // GroupByLC.
        if self.group_by_lc {
            let c = self.new[self.idx];
            while (i < len) && (self.old[i] != self.new[i]) && (self.new[i] == c) {
                i = i + 1;
            }

            let n = i - self.idx;
            self.idx = i;
            return Some(Group::Diff(start, n));
        }

        // GroupBySS2.
        if i + 1 >= len {
            self.idx = i + 1;
            return Some(Group::Diff(start, 1));
        } else {
            let c0 = self.new[self.idx + 0];
            let c1 = self.new[self.idx + 1];
            while i + 1 < len {
                if (self.old[i + 0] != self.new[i + 0] || self.old[i + 1] != self.new[i + 1])
                        && (self.new[i + 0] == c0 && self.new[i + 1] == c1) {
                    i = i + 2;
                } else {
                    break;
                }
            }

            let n = i - self.idx;
            self.idx = i;
            return Some(Group::Diff(start, n));
        }
    }
}

impl<'a> GroupByValue<'a> {
    /// Create a new GroupByValue iterator.
    fn new(buf: &'a [u8]) -> Self {
        GroupByValue {
            buf: buf,
            idx: 0,
        }
    }
}

impl<'a> Iterator for GroupByValue<'a> {
    type Item = Group;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Group> {
        let len = self.buf.len();
        let start = self.idx;
        let mut i = self.idx;

        if i >= len {
            return None;
        } else {
            let c = self.buf[self.idx];
            while (i < len) && (self.buf[i] == c) {
                i = i + 1;
            }

            let n = i - self.idx;
            self.idx = i;
            return Some(Group::Same(start, n));
        }
    }
}

impl LinScale {
    /// Create a new LinScale iterator.
    fn new(sw: usize, dw: usize) -> Self {
        assert!(sw > 0 && dw > 0);
        LinScale {
            sw: sw,
            dw: dw,
            sx: 0,
            dx: 0,
            xerr: sw / 2,
        }
    }
}

impl Iterator for LinScale {
    type Item = (usize, usize);

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Self::Item> {
        if self.dx >= self.dw {
            return None;
        }

        let res = (self.sx, self.dx);

        // We want to maintain the relationship: sx / sw = (dx + 0.5) / dw.
        // As we advance dx -> dx' = dx + 1,
        // then we advance sx -> sx' = sx + sw / dw.
        //
        // Accumulate truncation errors in xerr, and add it back in
        // when advancing sx -> sx' = sx + (sw + xerr) / dw.
        self.dx = self.dx + 1;
        if self.dx < self.dw {
            let (add, overflow) = self.sw.overflowing_add(self.xerr);
            if !overflow {
                let div = add / self.dw;
                let rem = add - div * self.dw;
                self.sx = self.sx + div;
                self.xerr = rem;
            } else {
                // Have total_add = (::std::usize::MAX + 1) + add,
                // add < ::std::usize::MAX.
                let add1 = add + 1;
                let div1 = add1 / self.dw;
                let rem1 = add1 - div1 * self.dw;
                let div2 = ::std::usize::MAX / self.dw;
                let rem2 = ::std::usize::MAX - div2 * self.dw;

                // We have rem1 < dw, rem2 < dw.
                // For rem1 + rem2 to overflow, need:
                //
                //  dw > (::std::usize::MAX + 1) / 2
                //
                // At this range of dw, div2 = 1, so:
                //
                //  rem2 = ::std::usize::MAX - dw
                //
                // Therefore to overflow, need:
                //
                //  rem1 + rem2 >= ::std::usize::MAX + 1
                //  rem1 >= (::std::usize::MAX + 1) - (::std::usize::MAX - dw)
                //        = dw + 1
                //
                // Since rem1 < dw, overflow cannot happen.
                let div3 = (rem1 + rem2) / self.dw;
                let rem3 = (rem1 + rem2) - div3 * self.dw;
                self.sx = self.sx + div1 + div2 + div3;
                self.xerr = rem3;
            }
        } else {
            self.sx = self.sw;
        }

        return Some(res);
    }
}

#[cfg(test)]
mod tests {
    use super::{Group,GroupByEq,GroupByLC,GroupBySS2,GroupByValue,LinScale};

    #[test]
    fn test_group_by_eq() {
        let xs = [ 1, 2, 3, 4, 5, 6, 7, 8, 9 ];
        let ys = [ 0, 0, 3, 4, 5, 0, 7, 8, 9 ];
        //         ^^^^  ^^^^^^^  ^  ^^^^^^^
        let expected = [
            Group::Same(0, 0), // prepend
            Group::Diff(0, 2), Group::Same(2, 3), Group::Diff(5, 1) ];

        let gs: Vec<Group>
            = GroupByEq::new(xs.iter(), ys.iter())
            .set_prepend_same_run()
            .set_ignore_final_same_run()
            .collect();

        assert_eq!(&gs[..], &expected[..]);
    }

    #[test]
    fn test_group_by_lc() {
        let xs = [ 1, 2, 3, 4, 5, 6, 7, 8, 9 ];
        let ys = [ 2, 1, 3, 4, 5, 0, 0, 0, 9 ];
        //         ^  ^  ^^^^^^^  ^^^^^^^  ^
        let expected = [
            Group::Same(0, 0), // prepend
            Group::Diff(0, 1), Group::Diff(1, 1), Group::Same(2, 3), Group::Diff(5, 3) ];

        let gs: Vec<Group>
            = GroupByLC::new_lc(&xs, &ys)
            .set_prepend_same_run()
            .set_ignore_final_same_run()
            .collect();

        assert_eq!(&gs[..], &expected[..]);
    }

    #[test]
    fn test_group_by_ss2() {
        let xs = [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 ];
        let ys = [ 2, 2, 3, 4, 5, 0, 1, 0, 1,  0 ];
        //         ^^^^  ^^^^^^^  ^^^^^^^^^^  ^^
        let expected = [
            Group::Same(0, 0), // prepend
            Group::Diff(0, 2), Group::Same(2, 3), Group::Diff(5, 4), Group::Diff(9, 1) ];

        let gs: Vec<Group>
            = GroupBySS2::new_ss2(&xs, &ys)
            .set_prepend_same_run()
            .set_ignore_final_same_run()
            .collect();

        assert_eq!(&gs[..], &expected[..]);
    }

    #[test]
    fn test_group_by_value() {
        let xs = [ 1, 1, 3, 4, 4, 4, 4, 7, 7 ];
        //         ^^^^  ^  ^^^^^^^^^^  ^^^^
        let expected = [
            Group::Same(0, 2), Group::Same(2, 1), Group::Same(3, 4), Group::Same(7, 2) ];

        let gs: Vec<Group>
            = GroupByValue::new(&xs).collect();

        assert_eq!(&gs[..], &expected[..]);
    }

    #[test]
    fn test_linscale() {
        fn linscale(sw: usize, dw: usize, dx: usize) -> usize {
            match dx {
                0 => 0,
                _ => (dx * sw + sw / 2) / dw,
            }
        }

        let sw = 320;
        let dw = 17;
        let expected: Vec<usize>
            = (0..dw)
            .map(|dx| linscale(sw, dw, dx))
            .collect();

        let xs: Vec<usize>
            = LinScale::new(sw, dw)
            .map(|x| x.0)
            .collect();

        assert_eq!(&xs[..], &expected[..]);
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn test_linscale_overflow() {
        // Expected results from Wolfram Alpha, for n > 0:
        //
        //  [(2^32-1) * n + (2^32-1) / 2] / 10
        let expected = [
              644_245_094,
            1_073_741_823,
            1_503_238_553,
            1_932_735_282,
            2_362_232_012,
            2_791_728_741,
            3_221_225_471,
            3_650_722_200,
            4_080_218_930 ];

        let xs: Vec<usize>
            = LinScale::new(::std::usize::MAX, 10)
            .map(|x| x.0)
            .collect();

        assert_eq!(&xs[1..], &expected[..]);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn test_linscale_overflow() {
        // Expected results from Wolfram Alpha, for n > 0:
        //
        //  [(2^64-1) * n + (2^64-1) / 2] / 10
        let expected = [
             2_767_011_611_056_432_742,
             4_611_686_018_427_387_903,
             6_456_360_425_798_343_065,
             8_301_034_833_169_298_226,
            10_145_709_240_540_253_388,
            11_990_383_647_911_208_549,
            13_835_058_055_282_163_711,
            15_679_732_462_653_118_872,
            17_524_406_870_024_074_034 ];

        let xs: Vec<usize>
            = LinScale::new(::std::usize::MAX, 10)
            .map(|x| x.0)
            .collect();

        assert_eq!(&xs[1..], &expected[..]);
    }
}
