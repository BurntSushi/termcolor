use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use regex::bytes::Regex;
use term::{self, Terminal};
use term::color::*;
use term::terminfo::TermInfo;

use terminal::TerminfoTerminal;
use types::FileTypeDef;

use self::Writer::*;

/// Printer encapsulates all output logic for searching.
///
/// Note that we currently ignore all write errors. It's probably worthwhile
/// to fix this, but printers are only ever used for writes to stdout or
/// writes to memory, neither of which commonly fail.
pub struct Printer<W> {
    /// The underlying writer.
    wtr: Writer<W>,
    /// Whether anything has been printed to wtr yet.
    has_printed: bool,
    /// Whether to show column numbers for the first match or not.
    column: bool,
    /// The string to use to separate non-contiguous runs of context lines.
    context_separator: Vec<u8>,
    /// The end-of-line terminator used by the printer. In general, eols are
    /// printed via the match directly, but occasionally we need to insert them
    /// ourselves (for example, to print a context separator).
    eol: u8,
    /// Whether to show file name as a heading or not.
    ///
    /// N.B. If with_filename is false, then this setting has no effect.
    heading: bool,
    /// Whether to suppress all output.
    quiet: bool,
    /// A string to use as a replacement of each match in a matching line.
    replace: Option<Vec<u8>>,
    /// Whether to prefix each match with the corresponding file name.
    with_filename: bool,
}

impl<W: Send + io::Write> Printer<W> {
    /// Create a new printer that writes to wtr.
    ///
    /// `color` should be true if the printer should try to use coloring.
    pub fn new(wtr: W, color: bool) -> Printer<W> {
        Printer {
            wtr: Writer::new(wtr, color),
            has_printed: false,
            column: false,
            context_separator: "--".to_string().into_bytes(),
            eol: b'\n',
            heading: false,
            quiet: false,
            replace: None,
            with_filename: false,
        }
    }

    /// When set, column numbers will be printed for the first match on each
    /// line.
    pub fn column(mut self, yes: bool) -> Printer<W> {
        self.column = yes;
        self
    }

    /// Set the context separator. The default is `--`.
    pub fn context_separator(mut self, sep: Vec<u8>) -> Printer<W> {
        self.context_separator = sep;
        self
    }

    /// Set the end-of-line terminator. The default is `\n`.
    pub fn eol(mut self, eol: u8) -> Printer<W> {
        self.eol = eol;
        self
    }

    /// Whether to show file name as a heading or not.
    ///
    /// N.B. If with_filename is false, then this setting has no effect.
    pub fn heading(mut self, yes: bool) -> Printer<W> {
        self.heading = yes;
        self
    }

    /// When set, all output is suppressed.
    pub fn quiet(mut self, yes: bool) -> Printer<W> {
        self.quiet = yes;
        self
    }

    /// Replace every match in each matching line with the replacement string
    /// given.
    ///
    /// The replacement string syntax is documented here:
    /// https://doc.rust-lang.org/regex/regex/bytes/struct.Captures.html#method.expand
    pub fn replace(mut self, replacement: Vec<u8>) -> Printer<W> {
        self.replace = Some(replacement);
        self
    }

    /// When set, each match is prefixed with the file name that it came from.
    pub fn with_filename(mut self, yes: bool) -> Printer<W> {
        self.with_filename = yes;
        self
    }

    /// Returns true if and only if something has been printed.
    pub fn has_printed(&self) -> bool {
        self.has_printed
    }

    /// Flushes the underlying writer and returns it.
    pub fn into_inner(mut self) -> W {
        let _ = self.wtr.flush();
        self.wtr.into_inner()
    }

    /// Prints a type definition.
    pub fn type_def(&mut self, def: &FileTypeDef) {
        self.write(def.name().as_bytes());
        self.write(b": ");
        let mut first = true;
        for pat in def.patterns() {
            if !first {
                self.write(b", ");
            }
            self.write(pat.as_bytes());
            first = false;
        }
        self.write_eol();
    }

    /// Prints the given path.
    pub fn path<P: AsRef<Path>>(&mut self, path: P) {
        self.write(path.as_ref().to_string_lossy().as_bytes());
        self.write_eol();
    }

    /// Prints the given path and a count of the number of matches found.
    pub fn path_count<P: AsRef<Path>>(&mut self, path: P, count: u64) {
        if self.with_filename {
            self.write(path.as_ref().to_string_lossy().as_bytes());
            self.write(b":");
        }
        self.write(count.to_string().as_bytes());
        self.write_eol();
    }

    /// Prints the context separator.
    pub fn context_separate(&mut self) {
        // N.B. We can't use `write` here because of borrowing restrictions.
        if self.quiet {
            return;
        }
        if self.context_separator.is_empty() {
            return;
        }
        self.has_printed = true;
        let _ = self.wtr.write_all(&self.context_separator);
        let _ = self.wtr.write_all(&[self.eol]);
    }

    pub fn matched<P: AsRef<Path>>(
        &mut self,
        re: &Regex,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        if self.heading && self.with_filename && !self.has_printed {
            self.write_heading(path.as_ref());
        } else if !self.heading && self.with_filename {
            self.write(path.as_ref().to_string_lossy().as_bytes());
            self.write(b":");
        }
        if let Some(line_number) = line_number {
            self.line_number(line_number, b':');
        }
        if self.column {
            let c = re.find(&buf[start..end]).map(|(s, _)| s + 1).unwrap_or(0);
            self.write(c.to_string().as_bytes());
            self.write(b":");
        }
        if self.replace.is_some() {
            let line = re.replace_all(
                &buf[start..end], &**self.replace.as_ref().unwrap());
            self.write(&line);
        } else {
            self.write_match(re, &buf[start..end]);
        }
        if buf[start..end].last() != Some(&self.eol) {
            self.write_eol();
        }
    }

    pub fn write_match(&mut self, re: &Regex, buf: &[u8]) {
        if !self.wtr.is_color() {
            self.write(buf);
            return;
        }
        let mut last_written = 0;
        for (s, e) in re.find_iter(buf) {
            self.write(&buf[last_written..s]);
            let _ = self.wtr.fg(RED);
            let _ = self.wtr.attr(term::Attr::Bold);
            self.write(&buf[s..e]);
            let _ = self.wtr.reset();
            last_written = e;
        }
        self.write(&buf[last_written..]);
    }

    pub fn context<P: AsRef<Path>>(
        &mut self,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        if self.heading && self.with_filename && !self.has_printed {
            self.write_heading(path.as_ref());
        } else if !self.heading && self.with_filename {
            self.write(path.as_ref().to_string_lossy().as_bytes());
            self.write(b"-");
        }
        if let Some(line_number) = line_number {
            self.line_number(line_number, b'-');
        }
        self.write(&buf[start..end]);
        if buf[start..end].last() != Some(&self.eol) {
            self.write_eol();
        }
    }

    fn write_heading<P: AsRef<Path>>(&mut self, path: P) {
        if self.wtr.is_color() {
            let _ = self.wtr.fg(GREEN);
            let _ = self.wtr.attr(term::Attr::Bold);
        }
        self.write(path.as_ref().to_string_lossy().as_bytes());
        self.write_eol();
        if self.wtr.is_color() {
            let _ = self.wtr.reset();
        }
    }

    fn line_number(&mut self, n: u64, sep: u8) {
        if self.wtr.is_color() {
            let _ = self.wtr.fg(BLUE);
            let _ = self.wtr.attr(term::Attr::Bold);
        }
        self.write(n.to_string().as_bytes());
        if self.wtr.is_color() {
            let _ = self.wtr.reset();
        }
        self.write(&[sep]);
    }

    fn write(&mut self, buf: &[u8]) {
        if self.quiet {
            return;
        }
        self.has_printed = true;
        let _ = self.wtr.write_all(buf);
    }

    fn write_eol(&mut self) {
        let eol = self.eol;
        self.write(&[eol]);
    }
}

enum Writer<W> {
    Colored(TerminfoTerminal<W>),
    NoColor(W),
}

lazy_static! {
    static ref TERMINFO: Option<Arc<TermInfo>> = {
        match term::terminfo::TermInfo::from_env() {
            Ok(info) => Some(Arc::new(info)),
            Err(err) => {
                debug!("error loading terminfo for coloring: {}", err);
                None
            }
        }
    };
}

impl<W: Send + io::Write> Writer<W> {
    fn new(wtr: W, color: bool) -> Writer<W> {
        // If we want color, build a TerminfoTerminal and see if the current
        // environment supports coloring. If not, bail with NoColor. To avoid
        // losing our writer (ownership), do this the long way.
        if !color || TERMINFO.is_none() {
            return NoColor(wtr);
        }
        let info = TERMINFO.clone().unwrap();
        let tt = TerminfoTerminal::new_with_terminfo(wtr, info);
        if !tt.supports_color() {
            debug!("environment doesn't support coloring");
            return NoColor(tt.into_inner());
        }
        Colored(tt)
    }

    fn is_color(&self) -> bool {
        match *self {
            Colored(_) => true,
            NoColor(_) => false,
        }
    }

    fn map_result<F>(
        &mut self,
        mut f: F,
    ) -> term::Result<()>
    where F: FnMut(&mut TerminfoTerminal<W>) -> term::Result<()> {
        match *self {
            Colored(ref mut w) => f(w),
            NoColor(_) => Err(term::Error::NotSupported),
        }
    }

    fn map_bool<F>(
        &self,
        mut f: F,
    ) -> bool
    where F: FnMut(&TerminfoTerminal<W>) -> bool {
        match *self {
            Colored(ref w) => f(w),
            NoColor(_) => false,
        }
    }
}

impl<W: Send + io::Write> io::Write for Writer<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            Colored(ref mut w) => w.write(buf),
            NoColor(ref mut w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            Colored(ref mut w) => w.flush(),
            NoColor(ref mut w) => w.flush(),
        }
    }
}

impl<W: Send + io::Write> term::Terminal for Writer<W> {
    type Output = W;

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

    fn get_ref(&self) -> &W {
        match *self {
            Colored(ref w) => w.get_ref(),
            NoColor(ref w) => w,
        }
    }

    fn get_mut(&mut self) -> &mut W {
        match *self {
            Colored(ref mut w) => w.get_mut(),
            NoColor(ref mut w) => w,
        }
    }

    fn into_inner(self) -> W {
        match self {
            Colored(w) => w.into_inner(),
            NoColor(w) => w,
        }
    }
}
