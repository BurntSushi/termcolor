use std::io::{self, Write};

/// Out controls the actual output of all search results for a particular file
/// to the end user.
///
/// (The difference between Out and Printer is that a Printer works with
/// individual search results where as Out works with search results for each
/// file as a whole. For example, it knows when to print a file separator.)
pub struct Out<W: io::Write> {
    wtr: io::BufWriter<W>,
    printed: bool,
    file_separator: Vec<u8>,
}

impl<W: io::Write> Out<W> {
    /// Create a new Out that writes to the wtr given.
    pub fn new(wtr: W) -> Out<W> {
        Out {
            wtr: io::BufWriter::new(wtr),
            printed: false,
            file_separator: vec![],
        }
    }

    /// If set, the separator is printed between matches from different files.
    /// By default, no separator is printed.
    ///
    /// If sep is empty, then no file separator is printed.
    pub fn file_separator(mut self, sep: Vec<u8>) -> Out<W> {
        self.file_separator = sep;
        self
    }

    /// Write the search results of a single file to the underlying wtr and
    /// flush wtr.
    pub fn write(&mut self, buf: &[u8]) {
        if self.printed && !self.file_separator.is_empty() {
            let _ = self.wtr.write_all(&self.file_separator);
            let _ = self.wtr.write_all(b"\n");
        }
        let _ = self.wtr.write_all(buf);
        let _ = self.wtr.flush();
        self.printed = true;
    }
}
