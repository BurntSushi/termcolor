/*!
The search module is responsible for searching a single file and printing
matches.
*/

use std::cmp;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use grep::{Grep, Match};
use memchr::{memchr, memrchr};

use printer::Printer;

/// The default read size (capacity of input buffer).
const READ_SIZE: usize = 8 * (1<<10);

/// Error describes errors that can occur while searching.
#[derive(Debug)]
pub enum Error {
    Io {
        err: io::Error,
        path: PathBuf,
    }
}

impl Error {
    fn from_io<P: AsRef<Path>>(err: io::Error, path: P) -> Error {
        Error::Io { err: err, path: path.as_ref().to_path_buf() }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io { ref err, .. } => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Io { ref err, .. } => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io { ref err, ref path } => {
                write!(f, "{}: {}", path.display(), err)
            }
        }
    }
}

pub struct Searcher<'a, R, W: 'a> {
    inp: &'a mut InputBuffer,
    printer: &'a mut Printer<W>,
    grep: &'a Grep,
    path: &'a Path,
    haystack: R,
    match_count: u64,
    line_count: Option<u64>,
    last_match: Match,
    last_printed: usize,
    last_line: usize,
    after_context_remaining: usize,
    count: bool,
    invert_match: bool,
    line_number: bool,
    before_context: usize,
    after_context: usize,
}

impl<'a, R: io::Read, W: io::Write> Searcher<'a, R, W> {
    /// Create a new searcher.
    ///
    /// `inp` is a reusable input buffer that is used as scratch space by this
    /// searcher.
    ///
    /// `printer` is used to output all results of searching.
    ///
    /// `grep` is the actual matcher.
    ///
    /// `path` is the file path being searched.
    ///
    /// `haystack` is a reader of text to search.
    pub fn new(
        inp: &'a mut InputBuffer,
        printer: &'a mut Printer<W>,
        grep: &'a Grep,
        path: &'a Path,
        haystack: R,
    ) -> Searcher<'a, R, W> {
        Searcher {
            inp: inp,
            printer: printer,
            grep: grep,
            path: path,
            haystack: haystack,
            match_count: 0,
            line_count: None,
            last_match: Match::default(),
            last_printed: 0,
            last_line: 0,
            after_context_remaining: 0,
            count: false,
            invert_match: false,
            line_number: false,
            before_context: 0,
            after_context: 0,
        }
    }

    /// If enabled, searching will print a count instead of each match.
    ///
    /// Disabled by default.
    pub fn count(mut self, yes: bool) -> Self {
        self.count = yes;
        self
    }

    /// If enabled, matching is inverted so that lines that *don't* match the
    /// given pattern are treated as matches.
    pub fn invert_match(mut self, yes: bool) -> Self {
        self.invert_match = yes;
        self
    }

    /// If enabled, compute line numbers and prefix each line of output with
    /// them.
    pub fn line_number(mut self, yes: bool) -> Self {
        self.line_number = yes;
        self
    }

    /// The number of contextual lines to show before each match. The default
    /// is zero.
    pub fn before_context(mut self, count: usize) -> Self {
        self.before_context = count;
        self
    }

    /// The number of contextual lines to show after each match. The default
    /// is zero.
    pub fn after_context(mut self, count: usize) -> Self {
        self.after_context = count;
        self
    }

    /// Execute the search. Results are written to the printer and the total
    /// number of matches is returned.
    #[inline(never)]
    pub fn run(mut self) -> Result<u64, Error> {
        self.inp.reset();
        self.match_count = 0;
        self.line_count = if self.line_number { Some(0) } else { None };
        self.last_match = Match::default();
        self.after_context_remaining = 0;
        loop {
            let upto = self.inp.lastnl;
            self.print_after_context(upto);
            if !try!(self.fill()) {
                break;
            }
            while self.inp.pos < self.inp.lastnl {
                let matched = self.grep.read_match(
                    &mut self.last_match,
                    &mut self.inp.buf[..self.inp.lastnl],
                    self.inp.pos);
                if self.invert_match {
                    let upto =
                        if matched {
                            self.last_match.start()
                        } else {
                            self.inp.lastnl
                        };
                    if upto > self.inp.pos {
                        let upto_context = self.inp.pos;
                        self.print_after_context(upto_context);
                        self.print_before_context(upto_context);
                        self.print_inverted_matches(upto);
                    }
                } else if matched {
                    self.match_count += 1;
                    if !self.count {
                        let start = self.last_match.start();
                        let end = self.last_match.end();
                        self.print_after_context(start);
                        self.print_before_context(start);
                        self.print_match(start, end);
                    }
                }
                if matched {
                    self.inp.pos = self.last_match.end() + 1;
                } else {
                    self.inp.pos = self.inp.lastnl;
                }
            }
        }
        if self.count && self.match_count > 0 {
            self.printer.path_count(self.path, self.match_count);
        }
        Ok(self.match_count)
    }

    fn fill(&mut self) -> Result<bool, Error> {
        let mut keep_from = self.inp.lastnl;
        if self.before_context > 0 || self.after_context > 0 {
            keep_from = start_of_previous_lines(
                &self.inp.buf,
                self.inp.lastnl.saturating_sub(1),
                cmp::max(self.before_context, self.after_context) + 1);
        }
        if keep_from < self.last_printed {
            self.last_printed = self.last_printed - keep_from;
        } else {
            self.last_printed = 0;
        }
        if keep_from < self.last_line {
            self.last_line = self.last_line - keep_from;
        } else {
            self.count_lines(keep_from);
            self.last_line = 0;
        }
        let ok = try!(self.inp.fill(&mut self.haystack, keep_from).map_err(|err| {
            Error::from_io(err, &self.path)
        }));
        Ok(ok)
    }

    #[inline(always)]
    fn print_inverted_matches(&mut self, upto: usize) {
        debug_assert!(self.invert_match);
        let mut it = IterLines::new(self.inp.pos);
        while let Some((start, end)) = it.next(&self.inp.buf[..upto]) {
            if !self.count {
                self.print_match(start, end);
            }
            self.inp.pos = end + 1;
            self.match_count += 1;
        }
    }

    #[inline(always)]
    fn print_before_context(&mut self, upto: usize) {
        if self.count || self.before_context == 0 {
            return;
        }
        let start = self.last_printed;
        let end = upto.saturating_sub(1);
        if start >= end {
            return;
        }
        let before_context_start =
            start + start_of_previous_lines(
                &self.inp.buf[start..],
                end - start,
                self.before_context);
        let mut it = IterLines::new(before_context_start);
        while let Some((s, e)) = it.next(&self.inp.buf[..end]) {
            self.print_separator(s);
            self.print_context(s, e);
        }
    }

    #[inline(always)]
    fn print_after_context(&mut self, upto: usize) {
        if self.count || self.after_context_remaining == 0 {
            return;
        }
        let start = self.last_printed;
        let end = upto;
        let mut it = IterLines::new(start);
        while let Some((s, e)) = it.next(&self.inp.buf[..end]) {
            self.print_context(s, e);
            self.after_context_remaining -= 1;
            if self.after_context_remaining == 0 {
                break;
            }
        }
    }

    #[inline(always)]
    fn print_match(&mut self, start: usize, end: usize) {
        let last_printed = cmp::min(end + 1, self.inp.lastnl);
        self.print_separator(start);
        self.count_lines(start);
        self.add_line(last_printed);
        self.printer.matched(
            &self.path, &self.inp.buf, start, end, self.line_count);
        self.last_printed = last_printed;
        self.after_context_remaining = self.after_context;
    }

    #[inline(always)]
    fn print_context(&mut self, start: usize, end: usize) {
        let last_printed = cmp::min(end + 1, self.inp.lastnl);
        self.count_lines(start);
        self.add_line(last_printed);
        self.printer.context(
            &self.path, &self.inp.buf, start, end, self.line_count);
        self.last_printed = last_printed;
    }

    #[inline(always)]
    fn print_separator(&mut self, before: usize) {
        if self.before_context == 0 && self.after_context == 0 {
            return;
        }
        if !self.printer.has_printed() {
            return;
        }
        if (self.last_printed == 0 && before > 0) || self.last_printed < before {
            self.printer.context_separator();
        }
    }

    #[inline(always)]
    fn count_lines(&mut self, upto: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += count_lines(&self.inp.buf[self.last_line..upto]);
            self.last_line = upto;
        }
    }

    #[inline(always)]
    fn add_line(&mut self, line_end: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += 1;
            self.last_line = line_end;
        }
    }
}

pub struct InputBuffer {
    read_size: usize,
    buf: Vec<u8>,
    tmp1: Vec<u8>,
    tmp2: Vec<u8>,
    pos: usize,
    lastnl: usize,
    end: usize,
    first: bool,
    is_binary: bool,
}

impl InputBuffer {
    /// Create a new buffer with a default capacity.
    pub fn new() -> InputBuffer {
        InputBuffer::with_capacity(READ_SIZE)
    }

    /// Create a new buffer with the capacity given.
    ///
    /// The capacity determines the size of each read from the underlying
    /// reader.
    ///
    /// `cap` must be a minimum of `1`.
    pub fn with_capacity(mut cap: usize) -> InputBuffer {
        if cap == 0 {
            cap = 1;
        }
        InputBuffer {
            read_size: cap,
            buf: vec![0; cap],
            tmp1: vec![],
            tmp2: vec![],
            pos: 0,
            lastnl: 0,
            end: 0,
            first: true,
            is_binary: false,
        }
    }

    fn reset(&mut self) {
        self.pos = 0;
        self.lastnl = 0;
        self.end = 0;
        self.first = true;
        self.is_binary = false;
    }

    fn fill<R: io::Read>(
        &mut self,
        rdr: &mut R,
        keep_from: usize,
    ) -> Result<bool, io::Error> {
        self.pos = 0;
        self.tmp1.clear();
        self.tmp2.clear();

        // Save the leftovers from the previous fill before anything else.
        if self.lastnl < self.end {
            self.tmp1.extend_from_slice(&self.buf[self.lastnl..self.end]);
        }
        // If we need to save lines to account for context, do that here.
        // These context lines have already been searched, but make up the
        // first bytes of this buffer.
        if keep_from < self.lastnl {
            self.tmp2.extend_from_slice(&self.buf[keep_from..self.lastnl]);
            self.buf[0..self.tmp2.len()].copy_from_slice(&self.tmp2);
            self.pos = self.tmp2.len();
        }
        if !self.tmp1.is_empty() {
            let (start, end) = (self.pos, self.pos + self.tmp1.len());
            self.buf[start..end].copy_from_slice(&self.tmp1);
            self.end = end;
        } else {
            self.end = self.pos;
        }
        self.lastnl = 0;
        while self.lastnl == 0 {
            if self.buf.len() - self.end < self.read_size {
                let min_len = self.read_size + self.buf.len() - self.end;
                let new_len = cmp::max(min_len, self.buf.len() * 2);
                self.buf.resize(new_len, 0);
            }
            let n = try!(rdr.read(
                &mut self.buf[self.end..self.end + self.read_size]));
            if self.first {
                if is_binary(&self.buf[self.end..self.end + n]) {
                    return Ok(false);
                }
            }
            self.first = false;
            if n == 0 {
                if self.end - self.pos == 0 {
                    return Ok(false);
                }
                self.lastnl = self.end;
                break;
            }
            // We know there is no nl between self.start..self.end since:
            //   1) If this is the first iteration, then any bytes preceding
            //      self.end do not contain nl by construction.
            //   2) Subsequent iterations only occur if no nl could be found.
            self.lastnl =
                memrchr(b'\n', &self.buf[self.end..self.end + n])
                .map(|i| self.end + i + 1)
                .unwrap_or(0);
            self.end += n;
        }
        Ok(true)
    }
}

/// Returns true if and only if the given buffer is determined to be "binary"
/// or otherwise not contain text data that is usefully searchable.
///
/// Note that this may return both false positives and false negatives!
#[inline(always)]
fn is_binary(buf: &[u8]) -> bool {
    if buf.len() >= 4 && &buf[0..4] == b"%PDF" {
        return true;
    }
    memchr(b'\x00', &buf[0..cmp::min(1024, buf.len())]).is_some()
}

/// Count the number of lines in the given buffer.
#[inline(always)]
fn count_lines(mut buf: &[u8]) -> u64 {
    let mut count = 0;
    while let Some(pos) = memchr(b'\n', buf) {
        count += 1;
        buf = &buf[pos + 1..];
    }
    count
}

/// An "iterator" over lines in a particular buffer.
///
/// Idiomatic Rust would borrow the buffer and use it as internal state to
/// advance over the positions of each line. We neglect that approach to avoid
/// the borrow in the search code. (Because the borrow prevents composition
/// through other mutable methods.)
struct IterLines {
    pos: usize,
}

impl IterLines {
    /// Creates a new iterator over lines starting at the position given.
    ///
    /// The buffer is passed to the `next` method.
    #[inline(always)]
    fn new(start: usize) -> IterLines {
        IterLines {
            pos: start,
        }
    }

    /// Return the start and end position of the next line in the buffer. The
    /// buffer given should be the same on every call.
    #[inline(always)]
    fn next(&mut self, buf: &[u8]) -> Option<(usize, usize)> {
        match memchr(b'\n', &buf[self.pos..]) {
            None => {
                if self.pos < buf.len() {
                    let start = self.pos;
                    self.pos = buf.len();
                    Some((start, buf.len()))
                } else {
                    None
                }
            }
            Some(end) => {
                let start = self.pos;
                let end = self.pos + end;
                self.pos = end + 1;
                Some((start, end))
            }
        }
    }
}

/// Returns the starting index of the Nth line preceding `end`.
///
/// If `buf` is empty, then `0` is returned. If `count` is `0`, then `end` is
/// returned.
///
/// If `end` points at a new line in `buf`, then searching starts as if `end`
/// pointed immediately before the new line.
///
/// The position returned corresponds to the first byte in the given line.
#[inline(always)]
fn start_of_previous_lines(
    buf: &[u8],
    mut end: usize,
    mut count: usize,
) -> usize {
    if buf.is_empty() {
        return 0;
    }
    if count == 0 {
        return end;
    }
    if end == buf.len() {
        end -= 1;
    }
    if buf[end] == b'\n' {
        if end == 0 {
            return end + 1;
        }
        end -= 1;
    }
    while count > 0 {
        if buf[end] == b'\n' {
            count -= 1;
            if count == 0 {
                return end + 1;
            }
            if end == 0 {
                return end;
            }
            end -= 1;
            continue;
        }
        match memrchr(b'\n', &buf[..end]) {
            None => {
                return 0;
            }
            Some(i) => {
                count -= 1;
                end = i;
                if end == 0 {
                    return end;
                }
                end -= 1;
            }
        }
    }
    end + 2
}

fn show(bytes: &[u8]) -> &str {
    ::std::str::from_utf8(bytes).unwrap()
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use grep::{Grep, GrepBuilder};

    use printer::Printer;

    use super::{InputBuffer, Searcher, start_of_previous_lines};

    fn hay(s: &str) -> io::Cursor<Vec<u8>> {
        io::Cursor::new(s.to_string().into_bytes())
    }

    fn matcher(pat: &str) -> Grep {
        GrepBuilder::new(pat).build().unwrap()
    }

    fn test_path() -> &'static Path {
        &Path::new("/baz.rs")
    }

    #[test]
    fn previous_lines() {
        let text = &b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
"[..];
        assert_eq!(366, text.len());

        assert_eq!(0, start_of_previous_lines(text, 366, 100));
        assert_eq!(366, start_of_previous_lines(text, 366, 0));

        assert_eq!(321, start_of_previous_lines(text, 366, 1));
        assert_eq!(321, start_of_previous_lines(text, 365, 1));
        assert_eq!(321, start_of_previous_lines(text, 364, 1));
        assert_eq!(321, start_of_previous_lines(text, 322, 1));
        assert_eq!(321, start_of_previous_lines(text, 321, 1));
        assert_eq!(258, start_of_previous_lines(text, 320, 1));

        assert_eq!(258, start_of_previous_lines(text, 366, 2));
        assert_eq!(258, start_of_previous_lines(text, 365, 2));
        assert_eq!(258, start_of_previous_lines(text, 364, 2));
        assert_eq!(258, start_of_previous_lines(text, 322, 2));
        assert_eq!(258, start_of_previous_lines(text, 321, 2));
        assert_eq!(193, start_of_previous_lines(text, 320, 2));

        assert_eq!(65, start_of_previous_lines(text, 66, 1));
        assert_eq!(0, start_of_previous_lines(text, 66, 2));
        assert_eq!(64, start_of_previous_lines(text, 64, 0));
        assert_eq!(0, start_of_previous_lines(text, 64, 1));
        assert_eq!(0, start_of_previous_lines(text, 64, 2));

        assert_eq!(0, start_of_previous_lines(text, 0, 2));
        assert_eq!(0, start_of_previous_lines(text, 0, 1));
    }

    #[test]
    fn previous_lines_short() {
        let text = &b"a\nb\nc\nd\ne\nf\n"[..];
        assert_eq!(12, text.len());

        assert_eq!(10, start_of_previous_lines(text, 12, 1));
        assert_eq!(8, start_of_previous_lines(text, 12, 2));
        assert_eq!(6, start_of_previous_lines(text, 12, 3));
        assert_eq!(4, start_of_previous_lines(text, 12, 4));
        assert_eq!(2, start_of_previous_lines(text, 12, 5));
        assert_eq!(0, start_of_previous_lines(text, 12, 6));
        assert_eq!(0, start_of_previous_lines(text, 12, 7));
        assert_eq!(10, start_of_previous_lines(text, 11, 1));
        assert_eq!(8, start_of_previous_lines(text, 11, 2));
        assert_eq!(6, start_of_previous_lines(text, 11, 3));
        assert_eq!(4, start_of_previous_lines(text, 11, 4));
        assert_eq!(2, start_of_previous_lines(text, 11, 5));
        assert_eq!(0, start_of_previous_lines(text, 11, 6));
        assert_eq!(0, start_of_previous_lines(text, 11, 7));
        assert_eq!(10, start_of_previous_lines(text, 10, 1));
        assert_eq!(8, start_of_previous_lines(text, 10, 2));
        assert_eq!(6, start_of_previous_lines(text, 10, 3));
        assert_eq!(4, start_of_previous_lines(text, 10, 4));
        assert_eq!(2, start_of_previous_lines(text, 10, 5));
        assert_eq!(0, start_of_previous_lines(text, 10, 6));
        assert_eq!(0, start_of_previous_lines(text, 10, 7));

        assert_eq!(8, start_of_previous_lines(text, 9, 1));
        assert_eq!(8, start_of_previous_lines(text, 8, 1));

        assert_eq!(6, start_of_previous_lines(text, 7, 1));
        assert_eq!(6, start_of_previous_lines(text, 6, 1));

        assert_eq!(4, start_of_previous_lines(text, 5, 1));
        assert_eq!(4, start_of_previous_lines(text, 4, 1));

        assert_eq!(2, start_of_previous_lines(text, 3, 1));
        assert_eq!(2, start_of_previous_lines(text, 2, 1));

        assert_eq!(0, start_of_previous_lines(text, 1, 1));
        assert_eq!(0, start_of_previous_lines(text, 0, 1));
    }

    #[test]
    fn previous_lines_empty() {
        let text = &b"\n\n\nd\ne\nf\n"[..];
        assert_eq!(9, text.len());

        assert_eq!(7, start_of_previous_lines(text, 9, 1));
        assert_eq!(5, start_of_previous_lines(text, 9, 2));
        assert_eq!(3, start_of_previous_lines(text, 9, 3));
        assert_eq!(2, start_of_previous_lines(text, 9, 4));
        assert_eq!(1, start_of_previous_lines(text, 9, 5));
        assert_eq!(0, start_of_previous_lines(text, 9, 6));
        assert_eq!(0, start_of_previous_lines(text, 9, 7));

        let text = &b"a\n\n\nd\ne\nf\n"[..];
        assert_eq!(10, text.len());

        assert_eq!(8, start_of_previous_lines(text, 10, 1));
        assert_eq!(6, start_of_previous_lines(text, 10, 2));
        assert_eq!(4, start_of_previous_lines(text, 10, 3));
        assert_eq!(3, start_of_previous_lines(text, 10, 4));
        assert_eq!(2, start_of_previous_lines(text, 10, 5));
        assert_eq!(0, start_of_previous_lines(text, 10, 6));
        assert_eq!(0, start_of_previous_lines(text, 10, 7));
    }

    #[test]
    fn basic_search() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.run().unwrap()
        };
        assert_eq!(2, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "\
/baz.rs:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn line_numbers() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.line_number(true).run().unwrap()
        };
        assert_eq!(2, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn count() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.count(true).run().unwrap()
        };
        assert_eq!(2, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "/baz.rs:2\n");
    }

    #[test]
    fn invert_match() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.invert_match(true).run().unwrap()
        };
        assert_eq!(4, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "\
/baz.rs:Holmeses, success in the province of detective work must always
/baz.rs:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn invert_match_line_numbers() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.invert_match(true).line_number(true).run().unwrap()
        };
        assert_eq!(4, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn invert_match_count() {
        let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
        let mut inp = InputBuffer::with_capacity(1);
        let mut pp = Printer::new(vec![]);
        let grep = matcher("Sherlock");
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), text);
            searcher.invert_match(true).count(true).run().unwrap()
        };
        assert_eq!(4, count);
        let out = String::from_utf8(pp.into_inner()).unwrap();
        assert_eq!(out, "/baz.rs:4\n");
    }

    macro_rules! before_context {
        ($name:ident, $query:expr, $num:expr, $expect:expr) => {
            before_context!($name, $query, $num, $expect, false);
        };
        ($name:ident, $query:expr, $num:expr, $expect:expr, $invert:expr) => {
            #[test]
            fn $name() {
                let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
                let mut inp = InputBuffer::with_capacity(4096);
                let mut pp = Printer::new(vec![]);
                let grep = matcher($query);
                let count = {
                    let mut searcher = Searcher::new(
                        &mut inp, &mut pp, &grep, test_path(), text);
                    searcher = searcher.line_number(true);
                    searcher = searcher.before_context($num);
                    searcher = searcher.invert_match($invert);
                    searcher.run().unwrap()
                };
                let out = String::from_utf8(pp.into_inner()).unwrap();
                assert_eq!(out, $expect);
            }
        };
    }

    before_context!(before_context_one, "Sherlock", 1, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");

    before_context!(before_context_invert_one1, "Sherlock", 1, "\
/baz.rs-1-For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
", true);

    before_context!(before_context_invert_one2, " a ", 1, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
--
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
", true);

    before_context!(before_context_two1, "Sherlock", 2, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");

    before_context!(before_context_two2, "dusted", 2, "\
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
");

    before_context!(before_context_two3, "success|attached", 2, "\
/baz.rs-1-For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
--
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");

    before_context!(before_context_three, "Sherlock", 3, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");

    macro_rules! after_context {
        ($name:ident, $query:expr, $num:expr, $expect:expr) => {
            after_context!($name, $query, $num, $expect, false);
        };
        ($name:ident, $query:expr, $num:expr, $expect:expr, $invert:expr) => {
            #[test]
            fn $name() {
                let text = hay("\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
");
                let mut inp = InputBuffer::with_capacity(4096);
                let mut pp = Printer::new(vec![]);
                let grep = matcher($query);
                let count = {
                    let mut searcher = Searcher::new(
                        &mut inp, &mut pp, &grep, test_path(), text);
                    searcher = searcher.line_number(true);
                    searcher = searcher.after_context($num);
                    searcher = searcher.invert_match($invert);
                    searcher.run().unwrap()
                };
                let out = String::from_utf8(pp.into_inner()).unwrap();
                assert_eq!(out, $expect);
            }
        };
    }

    after_context!(after_context_one, "Sherlock", 1, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
");

    after_context!(after_context_invert_one1, "Sherlock", 1, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
", true);

    after_context!(after_context_invert_one2, " a ", 1, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
--
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
", true);

    after_context!(after_context_two1, "Sherlock", 2, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
");

    after_context!(after_context_two2, "dusted", 2, "\
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");

    after_context!(after_context_two3, "success|attached", 2, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
--
/baz.rs:6:and exhibited clearly, with a label attached.
");

    after_context!(after_context_three, "Sherlock", 3, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");
}
