use std::io::{self, Write};

use term::{StdoutTerminal, Terminal};
#[cfg(windows)]
use term::WinConsole;

use printer::Writer;

/// Out controls the actual output of all search results for a particular file
/// to the end user.
///
/// (The difference between Out and Printer is that a Printer works with
/// individual search results where as Out works with search results for each
/// file as a whole. For example, it knows when to print a file separator.)
pub struct Out<W: io::Write> {
    wtr: io::BufWriter<W>,
    term: Option<Box<StdoutTerminal>>,
    printed: bool,
    file_separator: Option<Vec<u8>>,
}

/// This is like term::stdout, but on Windows always uses WinConsole instead
/// of trying for a TerminfoTerminal. This may be a mistake.
#[cfg(windows)]
fn term_stdout() -> Option<Box<StdoutTerminal>> {
    WinConsole::new(io::stdout())
        .ok()
        .map(|t| Box::new(t) as Box<StdoutTerminal>)
}

#[cfg(not(windows))]
fn term_stdout() -> Option<Box<StdoutTerminal>> {
    // We never use this crap on *nix.
    None
}

impl<W: io::Write> Out<W> {
    /// Create a new Out that writes to the wtr given.
    pub fn new(wtr: W) -> Out<W> {
        Out {
            wtr: io::BufWriter::new(wtr),
            term: term_stdout(),
            printed: false,
            file_separator: None,
        }
    }

    /// If set, the separator is printed between matches from different files.
    /// By default, no separator is printed.
    ///
    /// If sep is empty, then no file separator is printed.
    pub fn file_separator(mut self, sep: Vec<u8>) -> Out<W> {
        self.file_separator = Some(sep);
        self
    }

    /// Write the search results of a single file to the underlying wtr and
    /// flush wtr.
    pub fn write(&mut self, buf: &Writer<Vec<u8>>) {
        if let Some(ref sep) = self.file_separator {
            if self.printed {
                let _ = self.wtr.write_all(sep);
                let _ = self.wtr.write_all(b"\n");
            }
        }
        match *buf {
            Writer::Colored(ref tt) => {
                let _ = self.wtr.write_all(tt.get_ref());
            }
            Writer::Windows(ref w) => {
                match self.term {
                    None => {
                        let _ = self.wtr.write_all(w.get_ref());
                    }
                    Some(ref mut stdout) => {
                        w.print_stdout(stdout);
                    }
                }
            }
            Writer::NoColor(ref buf) => {
                let _ = self.wtr.write_all(buf);
            }
        }
        let _ = self.wtr.flush();
        self.printed = true;
    }
}
