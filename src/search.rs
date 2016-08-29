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
    pub grep: &'a Grep,
    pub path: &'a Path,
    pub haystack: R,
    pub inp: &'a mut InputBuffer,
    pub printer: &'a mut Printer<W>,
}

impl<'a, R: io::Read, W: io::Write> Searcher<'a, R, W> {
    #[inline(never)]
    pub fn run(mut self) -> Result<(), Error> {
        self.inp.reset();
        let mut mat = Match::default();
        loop {
            let ok = try!(self.inp.fill(&mut self.haystack).map_err(|err| {
                Error::from_io(err, &self.path)
            }));
            if !ok {
                return Ok(());
            }
            loop {
                let ok = self.grep.read_match(
                    &mut mat,
                    &mut self.inp.buf[..self.inp.lastnl],
                    self.inp.pos);
                if !ok {
                    break;
                }
                self.inp.pos = mat.end() + 1;
                self.printer.matched(self.path, &self.inp.buf, &mat);
            }
        }
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
    pub fn new() -> InputBuffer {
        InputBuffer {
            buf: vec![0; READ_SIZE],
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
                .map(|i| self.end + i)
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
