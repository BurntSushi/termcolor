use std::io::{self, Write};

use term::{self, Terminal};
#[cfg(not(windows))]
use term::terminfo::TermInfo;
#[cfg(windows)]
use term::WinConsole;

#[cfg(windows)]
use terminal_win::WindowsBuffer;

/// Out controls the actual output of all search results for a particular file
/// to the end user.
///
/// (The difference between Out and Printer is that a Printer works with
/// individual search results where as Out works with search results for each
/// file as a whole. For example, it knows when to print a file separator.)
pub struct Out {
    #[cfg(not(windows))]
    term: ColoredTerminal<term::TerminfoTerminal<io::BufWriter<io::Stdout>>>,
    #[cfg(windows)]
    term: ColoredTerminal<WinConsole<io::Stdout>>,
    printed: bool,
    file_separator: Option<Vec<u8>>,
}

impl Out {
    /// Create a new Out that writes to the wtr given.
    #[cfg(not(windows))]
    pub fn new(color: bool) -> Out {
        let wtr = io::BufWriter::new(io::stdout());
        Out {
            term: ColoredTerminal::new(wtr, color),
            printed: false,
            file_separator: None,
        }
    }

    /// Create a new Out that writes to the wtr given.
    #[cfg(windows)]
    pub fn new(color: bool) -> Out {
        Out {
            term: ColoredTerminal::new_stdout(color),
            printed: false,
            file_separator: None,
        }
    }

    /// If set, the separator is printed between matches from different files.
    /// By default, no separator is printed.
    ///
    /// If sep is empty, then no file separator is printed.
    pub fn file_separator(mut self, sep: Vec<u8>) -> Out {
        self.file_separator = Some(sep);
        self
    }

    /// Write the search results of a single file to the underlying wtr and
    /// flush wtr.
    #[cfg(not(windows))]
    pub fn write(
        &mut self,
        buf: &ColoredTerminal<term::TerminfoTerminal<Vec<u8>>>,
    ) {
        self.write_sep();
        match *buf {
            ColoredTerminal::Colored(ref tt) => {
                let _ = self.term.write_all(tt.get_ref());
            }
            ColoredTerminal::NoColor(ref buf) => {
                let _ = self.term.write_all(buf);
            }
        }
        self.write_done();
    }
    /// Write the search results of a single file to the underlying wtr and
    /// flush wtr.
    #[cfg(windows)]
    pub fn write(
        &mut self,
        buf: &ColoredTerminal<WindowsBuffer>,
    ) {
        self.write_sep();
        match *buf {
            ColoredTerminal::Colored(ref tt) => {
                tt.print_stdout(&mut self.term);
            }
            ColoredTerminal::NoColor(ref buf) => {
                let _ = self.term.write_all(buf);
            }
        }
        self.write_done();
    }

    fn write_sep(&mut self) {
        if let Some(ref sep) = self.file_separator {
            if self.printed {
                let _ = self.term.write_all(sep);
                let _ = self.term.write_all(b"\n");
            }
        }
    }

    fn write_done(&mut self) {
        let _ = self.term.flush();
        self.printed = true;
    }
}

/// ColoredTerminal provides optional colored output through the term::Terminal
/// trait. In particular, it will dynamically configure itself to use coloring
/// if it's available in the environment.
#[derive(Clone, Debug)]
pub enum ColoredTerminal<T: Terminal + Send> {
    Colored(T),
    NoColor(T::Output),
}

#[cfg(not(windows))]
impl<W: io::Write + Send> ColoredTerminal<term::TerminfoTerminal<W>> {
    /// Create a new output buffer.
    ///
    /// When color is true, the buffer will attempt to support coloring.
    pub fn new(wtr: W, color: bool) -> Self {
        lazy_static! {
            // Only pay for parsing the terminfo once.
            static ref TERMINFO: Option<TermInfo> = {
                match TermInfo::from_env() {
                    Ok(info) => Some(info),
                    Err(err) => {
                        debug!("error loading terminfo for coloring: {}", err);
                        None
                    }
                }
            };
        }
        // If we want color, build a term::TerminfoTerminal and see if the
        // current environment supports coloring. If not, bail with NoColor. To
        // avoid losing our writer (ownership), do this the long way.
        if !color {
            return ColoredTerminal::NoColor(wtr);
        }
        let terminfo = match *TERMINFO {
            None => return ColoredTerminal::NoColor(wtr),
            Some(ref ti) => {
                // Ug, this should go away with the next release of `term`.
                TermInfo {
                    names: ti.names.clone(),
                    bools: ti.bools.clone(),
                    numbers: ti.numbers.clone(),
                    strings: ti.strings.clone(),
                }
            }
        };
        let tt = term::TerminfoTerminal::new_with_terminfo(wtr, terminfo);
        if !tt.supports_color() {
            debug!("environment doesn't support coloring");
            return ColoredTerminal::NoColor(tt.into_inner());
        }
        ColoredTerminal::Colored(tt)
    }
}

#[cfg(not(windows))]
impl ColoredTerminal<term::TerminfoTerminal<Vec<u8>>> {
    /// Clear the give buffer of all search results such that it is reusable
    /// in another search.
    pub fn clear(&mut self) {
        match *self {
            ColoredTerminal::Colored(ref mut tt) => {
                tt.get_mut().clear();
            }
            ColoredTerminal::NoColor(ref mut buf) => {
                buf.clear();
            }
        }
    }
}

#[cfg(windows)]
impl ColoredTerminal<WindowsBuffer> {
    /// Create a new output buffer.
    ///
    /// When color is true, the buffer will attempt to support coloring.
    pub fn new_buffer(color: bool) -> Self {
        if !color {
            ColoredTerminal::NoColor(vec![])
        } else {
            ColoredTerminal::Colored(WindowsBuffer::new())
        }
    }

    /// Clear the give buffer of all search results such that it is reusable
    /// in another search.
    pub fn clear(&mut self) {
        match *self {
            ColoredTerminal::Colored(ref mut win) => win.clear(),
            ColoredTerminal::NoColor(ref mut buf) => buf.clear(),
        }
    }
}

#[cfg(windows)]
impl ColoredTerminal<WinConsole<io::Stdout>> {
    /// Create a new output buffer.
    ///
    /// When color is true, the buffer will attempt to support coloring.
    pub fn new_stdout(color: bool) -> Self {
        if !color {
            return ColoredTerminal::NoColor(io::stdout());
        }
        match WinConsole::new(io::stdout()) {
            Ok(win) => ColoredTerminal::Colored(win),
            Err(_) => ColoredTerminal::NoColor(io::stdout()),
        }
    }
}

impl<T: Terminal + Send> ColoredTerminal<T> {
    fn map_result<F>(
        &mut self,
        mut f: F,
    ) -> term::Result<()>
    where F: FnMut(&mut T) -> term::Result<()> {
        match *self {
            ColoredTerminal::Colored(ref mut w) => f(w),
            ColoredTerminal::NoColor(_) => Err(term::Error::NotSupported),
        }
    }

    fn map_bool<F>(
        &self,
        mut f: F,
    ) -> bool
    where F: FnMut(&T) -> bool {
        match *self {
            ColoredTerminal::Colored(ref w) => f(w),
            ColoredTerminal::NoColor(_) => false,
        }
    }
}

impl<T: Terminal + Send> io::Write for ColoredTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            ColoredTerminal::Colored(ref mut w) => w.write(buf),
            ColoredTerminal::NoColor(ref mut w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: Terminal + Send> term::Terminal for ColoredTerminal<T> {
    type Output = T::Output;

    fn fg(&mut self, fg: term::color::Color) -> term::Result<()> {
        self.map_result(|w| w.fg(fg))
    }

    fn bg(&mut self, bg: term::color::Color) -> term::Result<()> {
        self.map_result(|w| w.bg(bg))
    }

    fn attr(&mut self, attr: term::Attr) -> term::Result<()> {
        self.map_result(|w| w.attr(attr))
    }

    fn supports_attr(&self, attr: term::Attr) -> bool {
        self.map_bool(|w| w.supports_attr(attr))
    }

    fn reset(&mut self) -> term::Result<()> {
        self.map_result(|w| w.reset())
    }

    fn supports_reset(&self) -> bool {
        self.map_bool(|w| w.supports_reset())
    }

    fn supports_color(&self) -> bool {
        self.map_bool(|w| w.supports_color())
    }

    fn cursor_up(&mut self) -> term::Result<()> {
        self.map_result(|w| w.cursor_up())
    }

    fn delete_line(&mut self) -> term::Result<()> {
        self.map_result(|w| w.delete_line())
    }

    fn carriage_return(&mut self) -> term::Result<()> {
        self.map_result(|w| w.carriage_return())
    }

    fn get_ref(&self) -> &Self::Output {
        match *self {
            ColoredTerminal::Colored(ref w) => w.get_ref(),
            ColoredTerminal::NoColor(ref w) => w,
        }
    }

    fn get_mut(&mut self) -> &mut Self::Output {
        match *self {
            ColoredTerminal::Colored(ref mut w) => w.get_mut(),
            ColoredTerminal::NoColor(ref mut w) => w,
        }
    }

    fn into_inner(self) -> Self::Output {
        match self {
            ColoredTerminal::Colored(w) => w.into_inner(),
            ColoredTerminal::NoColor(w) => w,
        }
    }
}
