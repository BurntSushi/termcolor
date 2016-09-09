use std::io::{self, Write};
use std::sync::Arc;

use term::{self, Terminal};
use term::color::Color;
use term::terminfo::TermInfo;
#[cfg(windows)]
use term::WinConsole;

use terminal::TerminfoTerminal;

pub type StdoutTerminal = Box<Terminal<Output=io::Stdout> + Send>;

/// Gets a terminal that supports color if available.
#[cfg(windows)]
fn term_stdout(color: bool) -> StdoutTerminal {
    let stdout = io::stdout();
    WinConsole::new(stdout)
        .ok()
        .map(|t| Box::new(t) as StdoutTerminal)
        .unwrap_or_else(|| {
            let stdout = io::stdout();
            Box::new(NoColorTerminal::new(stdout)) as StdoutTerminal
        })
}

/// Gets a terminal that supports color if available.
#[cfg(not(windows))]
fn term_stdout(color: bool) -> StdoutTerminal {
    let stdout = io::stdout();
    if !color || TERMINFO.is_none() {
        Box::new(NoColorTerminal::new(stdout))
    } else {
        let info = TERMINFO.clone().unwrap();
        Box::new(TerminfoTerminal::new_with_terminfo(stdout, info))
    }
}

/// Out controls the actual output of all search results for a particular file
/// to the end user.
///
/// (The difference between Out and Printer is that a Printer works with
/// individual search results where as Out works with search results for each
/// file as a whole. For example, it knows when to print a file separator.)
pub struct Out {
    term: StdoutTerminal,
    printed: bool,
    file_separator: Option<Vec<u8>>,
}

impl Out {
    /// Create a new Out that writes to the wtr given.
    pub fn new(color: bool) -> Out {
        Out {
            term: term_stdout(color),
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
    pub fn write(&mut self, buf: &OutBuffer) {
        if let Some(ref sep) = self.file_separator {
            if self.printed {
                let _ = self.term.write_all(sep);
                let _ = self.term.write_all(b"\n");
            }
        }
        match *buf {
            OutBuffer::Colored(ref tt) => {
                let _ = self.term.write_all(tt.get_ref());
            }
            OutBuffer::Windows(ref w) => {
                w.print_stdout(&mut self.term);
            }
            OutBuffer::NoColor(ref buf) => {
                let _ = self.term.write_all(buf);
            }
        }
        let _ = self.term.flush();
        self.printed = true;
    }
}

/// OutBuffer corresponds to the final output buffer for search results. All
/// search results are written to a buffer and then a buffer is flushed to
/// stdout only after the full search has completed.
#[derive(Clone, Debug)]
pub enum OutBuffer {
    Colored(TerminfoTerminal<Vec<u8>>),
    Windows(WindowsBuffer),
    NoColor(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct WindowsBuffer {
    buf: Vec<u8>,
    pos: usize,
    colors: Vec<WindowsColor>,
}

#[derive(Clone, Debug)]
pub struct WindowsColor {
    pos: usize,
    opt: WindowsOption,
}

#[derive(Clone, Debug)]
pub enum WindowsOption {
    Foreground(Color),
    Background(Color),
    Reset,
}

lazy_static! {
    static ref TERMINFO: Option<Arc<TermInfo>> = {
        match TermInfo::from_env() {
            Ok(info) => Some(Arc::new(info)),
            Err(err) => {
                debug!("error loading terminfo for coloring: {}", err);
                None
            }
        }
    };
}

impl OutBuffer {
    /// Create a new output buffer.
    ///
    /// When color is true, the buffer will attempt to support coloring.
    pub fn new(color: bool) -> OutBuffer {
        // If we want color, build a TerminfoTerminal and see if the current
        // environment supports coloring. If not, bail with NoColor. To avoid
        // losing our writer (ownership), do this the long way.
        if !color {
            return OutBuffer::NoColor(vec![]);
        }
        if cfg!(windows) {
            return OutBuffer::Windows(WindowsBuffer {
                buf: vec![],
                pos: 0,
                colors: vec![]
            });
        }
        if TERMINFO.is_none() {
            return OutBuffer::NoColor(vec![]);
        }
        let info = TERMINFO.clone().unwrap();
        let tt = TerminfoTerminal::new_with_terminfo(vec![], info);
        if !tt.supports_color() {
            debug!("environment doesn't support coloring");
            return OutBuffer::NoColor(tt.into_inner());
        }
        OutBuffer::Colored(tt)
    }

    /// Clear the give buffer of all search results such that it is reusable
    /// in another search.
    pub fn clear(&mut self) {
        match *self {
            OutBuffer::Colored(ref mut tt) => {
                tt.get_mut().clear();
            }
            OutBuffer::Windows(ref mut win) => {
                win.buf.clear();
                win.colors.clear();
                win.pos = 0;
            }
            OutBuffer::NoColor(ref mut buf) => {
                buf.clear();
            }
        }
    }

    fn map_result<F, G>(
        &mut self,
        mut f: F,
        mut g: G,
    ) -> term::Result<()>
    where F: FnMut(&mut TerminfoTerminal<Vec<u8>>) -> term::Result<()>,
          G: FnMut(&mut WindowsBuffer) -> term::Result<()> {
        match *self {
            OutBuffer::Colored(ref mut w) => f(w),
            OutBuffer::Windows(ref mut w) => g(w),
            OutBuffer::NoColor(_) => Err(term::Error::NotSupported),
        }
    }

    fn map_bool<F, G>(
        &self,
        mut f: F,
        mut g: G,
    ) -> bool
    where F: FnMut(&TerminfoTerminal<Vec<u8>>) -> bool,
          G: FnMut(&WindowsBuffer) -> bool {
        match *self {
            OutBuffer::Colored(ref w) => f(w),
            OutBuffer::Windows(ref w) => g(w),
            OutBuffer::NoColor(_) => false,
        }
    }
}

impl io::Write for OutBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            OutBuffer::Colored(ref mut w) => w.write(buf),
            OutBuffer::Windows(ref mut w) => w.write(buf),
            OutBuffer::NoColor(ref mut w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl term::Terminal for OutBuffer {
    type Output = Vec<u8>;

    fn fg(&mut self, fg: term::color::Color) -> term::Result<()> {
        self.map_result(|w| w.fg(fg), |w| w.fg(fg))
    }

    fn bg(&mut self, bg: term::color::Color) -> term::Result<()> {
        self.map_result(|w| w.bg(bg), |w| w.bg(bg))
    }

    fn attr(&mut self, attr: term::Attr) -> term::Result<()> {
        self.map_result(|w| w.attr(attr), |w| w.attr(attr))
    }

    fn supports_attr(&self, attr: term::Attr) -> bool {
        self.map_bool(|w| w.supports_attr(attr), |w| w.supports_attr(attr))
    }

    fn reset(&mut self) -> term::Result<()> {
        self.map_result(|w| w.reset(), |w| w.reset())
    }

    fn supports_reset(&self) -> bool {
        self.map_bool(|w| w.supports_reset(), |w| w.supports_reset())
    }

    fn supports_color(&self) -> bool {
        self.map_bool(|w| w.supports_color(), |w| w.supports_color())
    }

    fn cursor_up(&mut self) -> term::Result<()> {
        self.map_result(|w| w.cursor_up(), |w| w.cursor_up())
    }

    fn delete_line(&mut self) -> term::Result<()> {
        self.map_result(|w| w.delete_line(), |w| w.delete_line())
    }

    fn carriage_return(&mut self) -> term::Result<()> {
        self.map_result(|w| w.carriage_return(), |w| w.carriage_return())
    }

    fn get_ref(&self) -> &Vec<u8> {
        match *self {
            OutBuffer::Colored(ref w) => w.get_ref(),
            OutBuffer::Windows(ref w) => w.get_ref(),
            OutBuffer::NoColor(ref w) => w,
        }
    }

    fn get_mut(&mut self) -> &mut Vec<u8> {
        match *self {
            OutBuffer::Colored(ref mut w) => w.get_mut(),
            OutBuffer::Windows(ref mut w) => w.get_mut(),
            OutBuffer::NoColor(ref mut w) => w,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        match self {
            OutBuffer::Colored(w) => w.into_inner(),
            OutBuffer::Windows(w) => w.into_inner(),
            OutBuffer::NoColor(w) => w,
        }
    }
}

impl WindowsBuffer {
    fn push(&mut self, opt: WindowsOption) {
        let pos = self.pos;
        self.colors.push(WindowsColor { pos: pos, opt: opt });
    }
}

impl WindowsBuffer {
    /// Print the contents to the given terminal.
    pub fn print_stdout(&self, tt: &mut StdoutTerminal) {
        if !tt.supports_color() {
            let _ = tt.write_all(&self.buf);
            let _ = tt.flush();
            return;
        }
        let mut last = 0;
        for col in &self.colors {
            let _ = tt.write_all(&self.buf[last..col.pos]);
            match col.opt {
                WindowsOption::Foreground(c) => {
                    let _ = tt.fg(c);
                }
                WindowsOption::Background(c) => {
                    let _ = tt.bg(c);
                }
                WindowsOption::Reset => {
                    let _ = tt.reset();
                }
            }
            last = col.pos;
        }
        let _ = tt.write_all(&self.buf[last..]);
        let _ = tt.flush();
    }
}

impl io::Write for WindowsBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = try!(self.buf.write(buf));
        self.pos += n;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl term::Terminal for WindowsBuffer {
    type Output = Vec<u8>;

    fn fg(&mut self, fg: term::color::Color) -> term::Result<()> {
        self.push(WindowsOption::Foreground(fg));
        Ok(())
    }

    fn bg(&mut self, bg: term::color::Color) -> term::Result<()> {
        self.push(WindowsOption::Background(bg));
        Ok(())
    }

    fn attr(&mut self, attr: term::Attr) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn supports_attr(&self, attr: term::Attr) -> bool {
        false
    }

    fn reset(&mut self) -> term::Result<()> {
        self.push(WindowsOption::Reset);
        Ok(())
    }

    fn supports_reset(&self) -> bool {
        true
    }

    fn supports_color(&self) -> bool {
        true
    }

    fn cursor_up(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn delete_line(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn carriage_return(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn get_ref(&self) -> &Vec<u8> {
        &self.buf
    }

    fn get_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

/// NoColorTerminal implements Terminal, but supports no coloring.
///
/// Its useful when an API requires a Terminal, but coloring isn't needed.
pub struct NoColorTerminal<W> {
    wtr: W,
}

impl<W: Send + io::Write> NoColorTerminal<W> {
    /// Wrap the given writer in a Terminal interface.
    pub fn new(wtr: W) -> NoColorTerminal<W> {
        NoColorTerminal {
            wtr: wtr,
        }
    }
}

impl<W: Send + io::Write> io::Write for NoColorTerminal<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.wtr.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wtr.flush()
    }
}

impl<W: Send + io::Write> term::Terminal for NoColorTerminal<W> {
    type Output = W;

    fn fg(&mut self, fg: term::color::Color) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn bg(&mut self, bg: term::color::Color) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn attr(&mut self, attr: term::Attr) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn supports_attr(&self, attr: term::Attr) -> bool {
        false
    }

    fn reset(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn supports_reset(&self) -> bool {
        false
    }

    fn supports_color(&self) -> bool {
        false
    }

    fn cursor_up(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn delete_line(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn carriage_return(&mut self) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn get_ref(&self) -> &W {
        &self.wtr
    }

    fn get_mut(&mut self) -> &mut W {
        &mut self.wtr
    }

    fn into_inner(self) -> W {
        self.wtr
    }
}
