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
    /// Normal IO or Mmap errors suck. Include the path the originated them.
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
    count: bool,
    invert_match: bool,
    line_number: bool,
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
            count: false,
            invert_match: false,
            line_number: false,
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

    /// Execute the search. Results are written to the printer and the total
    /// number of matches is returned.
    #[inline(never)]
    pub fn run(mut self) -> Result<u64, Error> {
        self.inp.reset();
        self.match_count = 0;
        self.line_count = if self.line_number { Some(0) } else { None };
        self.last_match = Match::default();
        loop {
            let ok = try!(self.inp.fill(&mut self.haystack).map_err(|err| {
                Error::from_io(err, &self.path)
            }));
            if !ok {
                break;
            }
            while self.inp.pos < self.inp.lastnl {
                let ok = self.grep.read_match(
                    &mut self.last_match,
                    &mut self.inp.buf[..self.inp.lastnl],
                    self.inp.pos);
                if !ok {
                    let upto = self.inp.lastnl;
                    if self.invert_match {
                        self.find_inverted_matches(upto);
                    } else {
                        self.count_lines(upto);
                    }
                    self.inp.pos = upto;
                    break;
                }
                if self.invert_match {
                    let inverted_upto = self.last_match.start();
                    self.find_inverted_matches(inverted_upto);
                    // Add a line to account for the match...
                    if let Some(ref mut line_count) = self.line_count {
                        *line_count += 1;
                    }
                    // ... and skip over the match.
                    self.inp.pos = self.last_match.end() + 1;
                } else {
                    self.match_count += 1;
                    if !self.count {
                        let upto = cmp::min(
                            self.inp.lastnl, self.last_match.end() + 1);
                        self.count_lines(upto);
                        self.printer.matched(
                            self.path,
                            &self.inp.buf,
                            self.last_match.start(),
                            self.last_match.end(),
                            self.line_count,
                        );
                    }
                    // Move the position one past the end of the match so that
                    // the next search starts after the nl. If we're at EOF,
                    // then pos will be past EOF.
                    self.inp.pos = self.last_match.end() + 1;
                }
            }
        }
        if self.count && self.match_count > 0 {
            self.printer.path_count(self.path, self.match_count);
        }
        Ok(self.match_count)
    }

    #[inline(always)]
    fn find_inverted_matches(&mut self, upto: usize) {
        debug_assert!(self.invert_match);
        while self.inp.pos < upto {
            let pos = memchr(b'\n', &self.inp.buf[self.inp.pos..upto])
                      .unwrap_or(upto);
            if !self.count {
                if let Some(ref mut line_count) = self.line_count {
                    *line_count += 1;
                }
                self.printer.matched(
                    &self.path,
                    &self.inp.buf,
                    self.inp.pos,
                    self.inp.pos + pos,
                    self.line_count,
                );
            }
            self.inp.pos += pos + 1;
            self.match_count += 1;
        }
    }

    #[inline(always)]
    fn count_lines(&mut self, upto: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += count_lines(&self.inp.buf[self.inp.pos..upto]);
        }
    }
}

pub struct InputBuffer {
    read_size: usize,
    buf: Vec<u8>,
    tmp: Vec<u8>,
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
    pub fn with_capacity(mut cap: usize) -> InputBuffer {
        if cap == 0 {
            cap = 1;
        }
        InputBuffer {
            read_size: cap,
            buf: vec![0; cap],
            tmp: vec![],
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

    fn fill<R: io::Read>(&mut self, rdr: &mut R) -> Result<bool, io::Error> {
        if self.lastnl < self.end {
            self.tmp.clear();
            self.tmp.extend_from_slice(&self.buf[self.lastnl..self.end]);
            self.buf[0..self.tmp.len()].copy_from_slice(&self.tmp);
            self.end = self.tmp.len();
        } else {
            self.end = 0;
        }
        self.pos = 0;
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
                if self.end == 0 {
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

#[inline(always)]
fn is_binary(buf: &[u8]) -> bool {
    if buf.len() >= 4 && &buf[0..4] == b"%PDF" {
        return true;
    }
    memchr(b'\x00', &buf[0..cmp::min(1024, buf.len())]).is_some()
}

#[inline(always)]
fn count_lines(mut buf: &[u8]) -> u64 {
    let mut count = 0;
    while let Some(pos) = memchr(b'\n', buf) {
        count += 1;
        buf = &buf[pos + 1..];
    }
    count
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use grep::{Grep, GrepBuilder};

    use printer::Printer;

    use super::{InputBuffer, Searcher};

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
}
