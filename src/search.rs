/*!
The search module is responsible for searching a single file and printing
matches.
*/

use std::cmp;
use std::error::Error as StdError;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use grep::Grep;
use memchr::memchr;
use memmap::{Mmap, Protection};

use printer::Printer;

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

/// Searcher searches a memory mapped buffer.
///
/// The `'g` lifetime refers to the lifetime of the underlying matcher.
pub struct Searcher<'g> {
    grep: &'g Grep,
    path: PathBuf,
    mmap: Option<Mmap>,
}

impl<'g> Searcher<'g> {
    /// Create a new memory map based searcher using the given matcher for the
    /// file path given.
    pub fn new<P: AsRef<Path>>(
        grep: &'g Grep,
        path: P,
    ) -> Result<Searcher<'g>, Error> {
        let file = try!(File::open(&path).map_err(|err| {
            Error::from_io(err, &path)
        }));
        let md = try!(file.metadata().map_err(|err| {
            Error::from_io(err, &path)
        }));
        let mmap =
            if md.len() == 0 {
                None
            } else {
                Some(try!(Mmap::open(&file, Protection::Read).map_err(|err| {
                    Error::from_io(err, &path)
                })))
            };
        Ok(Searcher {
            grep: grep,
            path: path.as_ref().to_path_buf(),
            mmap: mmap,
        })
    }

    /// Execute the search, writing the results to the printer given and
    /// returning the underlying buffer.
    pub fn search<W: io::Write>(&self, printer: Printer<W>) -> W {
        Search {
            grep: &self.grep,
            path: &*self.path,
            buf: self.buf(),
            printer: printer,
        }.run()
    }

    /// Execute the search, returning a count of the number of hits.
    pub fn count(&self) -> u64 {
        self.grep.iter(self.buf()).count() as u64
    }

    fn buf(&self) -> &[u8] {
        self.mmap.as_ref().map(|m| unsafe { m.as_slice() }).unwrap_or(&[])
    }
}

struct Search<'a, W> {
    grep: &'a Grep,
    path: &'a Path,
    buf: &'a [u8],
    printer: Printer<W>,
}

impl<'a, W: io::Write> Search<'a, W> {
    fn run(mut self) -> W {
        let is_binary = self.is_binary();
        let mut it = self.grep.iter(self.buf).peekable();
        if is_binary && it.peek().is_some() {
            self.printer.binary_matched(self.path);
            return self.printer.into_inner();
        }
        for m in it {
            self.printer.matched(self.path, self.buf, &m);
        }
        self.printer.into_inner()
    }

    fn is_binary(&self) -> bool {
        if self.buf.len() >= 4 && &self.buf[0..4] == b"%PDF" {
            return true;
        }
        memchr(b'\x00', &self.buf[0..cmp::min(1024, self.buf.len())]).is_some()
    }
}
