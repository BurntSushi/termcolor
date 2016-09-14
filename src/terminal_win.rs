/*!
This module contains a Windows-only *in-memory* implementation of the
`term::Terminal` trait.

This particular implementation is a bit idiosyncratic, and the "in-memory"
specification is to blame. In particular, on Windows, coloring requires
communicating with the console synchronously as data is written to stdout.
This is anathema to how ripgrep fundamentally works: by writing search results
to intermediate thread local buffers in order to maximize parallelism.

Eliminating parallelism on Windows isn't an option, because that would negate
a tremendous performance benefit just for coloring.

We've worked around this by providing an implementation of `term::Terminal`
that records precisely where a color or a reset should be invoked, according
to a byte offset in the in memory buffer. When the buffer is actually printed,
we copy the bytes from the buffer to stdout incrementally while invoking the
corresponding console APIs for coloring at the right location.

(Another approach would be to do ANSI coloring unconditionally, then parse that
and translate it to console commands. The advantage of that approach is that
it doesn't require any additional memory for storing offsets. In practice
though, coloring is only used in the terminal, which tends to correspond to
searches that produce very few results with respect to the corpus searched.
Therefore, this is an acceptable trade off. Namely, we do not pay for it when
coloring is disabled.
*/
use std::io;

use term::{self, Terminal};
use term::color::Color;

/// An in-memory buffer that provides Windows console coloring.
#[derive(Clone, Debug)]
pub struct WindowsBuffer {
    buf: Vec<u8>,
    pos: usize,
    colors: Vec<WindowsColor>,
}

/// A color associated with a particular location in a buffer.
#[derive(Clone, Debug)]
struct WindowsColor {
    pos: usize,
    opt: WindowsOption,
}

/// A color or reset directive that can be translated into an instruction to
/// the Windows console.
#[derive(Clone, Debug)]
enum WindowsOption {
    Foreground(Color),
    Background(Color),
    Reset,
}

impl WindowsBuffer {
    /// Create a new empty buffer for Windows console coloring.
    pub fn new() -> WindowsBuffer {
        WindowsBuffer {
            buf: vec![],
            pos: 0,
            colors: vec![],
        }
    }

    fn push(&mut self, opt: WindowsOption) {
        let pos = self.pos;
        self.colors.push(WindowsColor { pos: pos, opt: opt });
    }

    /// Print the contents to the given terminal.
    pub fn print_stdout<T: Terminal + Send>(&self, tt: &mut T) {
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

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.colors.clear();
        self.pos = 0;
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

impl Terminal for WindowsBuffer {
    type Output = Vec<u8>;

    fn fg(&mut self, fg: Color) -> term::Result<()> {
        self.push(WindowsOption::Foreground(fg));
        Ok(())
    }

    fn bg(&mut self, bg: Color) -> term::Result<()> {
        self.push(WindowsOption::Background(bg));
        Ok(())
    }

    fn attr(&mut self, _attr: term::Attr) -> term::Result<()> {
        Err(term::Error::NotSupported)
    }

    fn supports_attr(&self, _attr: term::Attr) -> bool {
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
