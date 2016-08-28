use std::io;
use std::path::Path;

use grep::Match;

macro_rules! wln {
    ($($tt:tt)*) => {
        let _ = writeln!($($tt)*);
    }
}

pub struct Printer<W> {
    wtr: W,
}

impl<W: io::Write> Printer<W> {
    pub fn new(wtr: W) -> Printer<W> {
        Printer {
            wtr: wtr,
        }
    }

    pub fn into_inner(self) -> W {
        self.wtr
    }

    pub fn path<P: AsRef<Path>>(&mut self, path: P) {
        wln!(&mut self.wtr, "{}", path.as_ref().display());
    }

    pub fn count(&mut self, count: u64) {
        wln!(&mut self.wtr, "{}", count);
    }

    pub fn matched<P: AsRef<Path>>(
        &mut self,
        path: P,
        buf: &[u8],
        m: &Match,
    ) {
        let _ = self.wtr.write(path.as_ref().to_string_lossy().as_bytes());
        let _ = self.wtr.write(b":");
        let _ = self.wtr.write(&buf[m.start()..m.end()]);
        let _ = self.wtr.write(b"\n");
    }

    pub fn binary_matched<P: AsRef<Path>>(&mut self, path: P) {
        wln!(&mut self.wtr, "binary file {} matches", path.as_ref().display());
    }
}
