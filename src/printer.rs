use std::io;
use std::path::Path;

macro_rules! wln {
    ($($tt:tt)*) => {
        let _ = writeln!($($tt)*);
    }
}

macro_rules! w {
    ($($tt:tt)*) => {
        let _ = write!($($tt)*);
    }
}

pub struct Printer<W> {
    wtr: W,
    has_printed: bool,
}

impl<W: io::Write> Printer<W> {
    pub fn new(wtr: W) -> Printer<W> {
        Printer {
            wtr: wtr,
            has_printed: false,
        }
    }

    pub fn has_printed(&self) -> bool {
        self.has_printed
    }

    pub fn into_inner(self) -> W {
        self.wtr
    }

    pub fn path<P: AsRef<Path>>(&mut self, path: P) {
        wln!(&mut self.wtr, "{}", path.as_ref().display());
    }

    pub fn path_count<P: AsRef<Path>>(&mut self, path: P, count: u64) {
        wln!(&mut self.wtr, "{}:{}", path.as_ref().display(), count);
    }

    pub fn count(&mut self, count: u64) {
        wln!(&mut self.wtr, "{}", count);
    }

    pub fn context_separator(&mut self) {
        wln!(&mut self.wtr, "--");
    }

    pub fn matched<P: AsRef<Path>>(
        &mut self,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        self.write(path.as_ref().to_string_lossy().as_bytes());
        self.write(b":");
        if let Some(line_number) = line_number {
            self.write(line_number.to_string().as_bytes());
            self.write(b":");
        }
        self.write(&buf[start..end]);
        self.write(b"\n");
    }

    pub fn context<P: AsRef<Path>>(
        &mut self,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        self.write(path.as_ref().to_string_lossy().as_bytes());
        self.write(b"-");
        if let Some(line_number) = line_number {
            self.write(line_number.to_string().as_bytes());
            self.write(b"-");
        }
        self.write(&buf[start..end]);
        self.write(b"\n");
    }

    pub fn binary_matched<P: AsRef<Path>>(&mut self, path: P) {
        wln!(&mut self.wtr, "binary file {} matches", path.as_ref().display());
    }

    fn write(&mut self, buf: &[u8]) {
        self.has_printed = true;
        let _ = self.wtr.write_all(buf);
    }
}
