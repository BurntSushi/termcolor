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
        let mut match_count = 0;
        let mut line_count = if self.line_number { Some(0) } else { None };
        let mut mat = Match::default();
        loop {
            let ok = try!(self.inp.fill(&mut self.haystack).map_err(|err| {
                Error::from_io(err, &self.path)
            }));
            if !ok {
                break;
            }
            while self.inp.pos < self.inp.lastnl {
                let ok = self.grep.read_match(
                    &mut mat,
                    &mut self.inp.buf[..self.inp.lastnl],
                    self.inp.pos);
                if !ok {
                    if self.invert_match {
                        while let Some(pos) = memchr(b'\n', &self.inp.buf[self.inp.pos..self.inp.lastnl]) {
                            if let Some(ref mut line_count) = line_count {
                                *line_count += 1;
                            }
                            self.printer.matched(
                                &self.path,
                                &self.inp.buf,
                                self.inp.pos,
                                self.inp.pos + pos,
                                line_count,
                            );
                            self.inp.pos += pos + 1;
                            match_count += 1;
                            if self.inp.pos >= self.inp.lastnl {
                                break;
                            }
                        }
                        self.inp.pos = self.inp.lastnl;
                    } else if let Some(ref mut line_count) = line_count {
                        *line_count += count_lines(
                            &self.inp.buf[self.inp.pos..self.inp.lastnl]);
                    }
                    break;
                }
                if self.invert_match {
                    while let Some(pos) = memchr(b'\n', &self.inp.buf[self.inp.pos..mat.start()]) {
                        if let Some(ref mut line_count) = line_count {
                            *line_count += 1;
                        }
                        self.printer.matched(
                            &self.path,
                            &self.inp.buf,
                            self.inp.pos,
                            self.inp.pos + pos,
                            line_count,
                        );
                        self.inp.pos += pos + 1;
                        match_count += 1;
                    }
                    if let Some(ref mut line_count) = line_count {
                        *line_count += 1;
                    }
                    self.inp.pos = mat.end() + 1;
                } else {
                    if let Some(ref mut line_count) = line_count {
                        // mat.end() always points immediately after the end
                        // of a match, which could be *at* a nl or past our
                        // current search buffer. Either way, count it as one
                        // more line.
                        *line_count += 1 + count_lines(
                            &self.inp.buf[self.inp.pos..mat.end()]);
                    }
                    match_count += 1;
                    if !self.count {
                        self.printer.matched(
                            self.path,
                            &self.inp.buf,
                            mat.start(),
                            mat.end(),
                            line_count,
                        );
                    }
                    // Move the position one past the end of the match so that
                    // the next search starts after the nl. If we're at EOF,
                    // then pos will be past EOF.
                    self.inp.pos = mat.end() + 1;
                }
            }
        }
        if self.count && match_count > 0 {
            self.printer.path_count(self.path, match_count);
        }
        Ok(match_count)
    }
}

pub struct InputBuffer {
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
    pub fn with_capacity(cap: usize) -> InputBuffer {
        InputBuffer {
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
            if self.buf.len() - self.end < READ_SIZE {
                let min_len = READ_SIZE + self.buf.len() - self.end;
                let new_len = cmp::max(min_len, self.buf.len() * 2);
                self.buf.resize(new_len, 0);
            }
            let n = try!(rdr.read(
                &mut self.buf[self.end..self.end + READ_SIZE]));
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

fn is_binary(buf: &[u8]) -> bool {
    if buf.len() >= 4 && &buf[0..4] == b"%PDF" {
        return true;
    }
    memchr(b'\x00', &buf[0..cmp::min(1024, buf.len())]).is_some()
}

fn count_lines(mut buf: &[u8]) -> u64 {
    let mut count = 0;
    while let Some(pos) = memchr(b'\n', buf) {
        count += 1;
        buf = &buf[pos + 1..];
    }
    count
}
