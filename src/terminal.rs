/*!
This module contains an implementation of the `term::Terminal` trait.

The actual implementation is copied almost verbatim from the `term` crate, so
this code is under the same license (MIT/Apache).

The specific reason why this is copied here is to wrap an Arc<TermInfo> instead
of a TermInfo. This makes multithreaded sharing much more performant.

N.B. This is temporary until this issue is fixed:
https://github.com/Stebalien/term/issues/64
*/

use std::io::{self, Write};
use std::sync::Arc;

use term::{Attr, Error, Result, Terminal, color};
use term::terminfo::TermInfo;
use term::terminfo::parm::{Param, Variables, expand};

/// A Terminal that knows how many colors it supports, with a reference to its
/// parsed Terminfo database record.
#[derive(Clone, Debug)]
pub struct TerminfoTerminal<T> {
    num_colors: u16,
    out: T,
    ti: Arc<TermInfo>,
}

impl<T: Write + Send> Terminal for TerminfoTerminal<T> {
    type Output = T;
    fn fg(&mut self, color: color::Color) -> Result<()> {
        let color = self.dim_if_necessary(color);
        if self.num_colors > color {
            return apply_cap(&self.ti, "setaf", &[Param::Number(color as i32)], &mut self.out);
        }
        Err(Error::ColorOutOfRange)
    }

    fn bg(&mut self, color: color::Color) -> Result<()> {
        let color = self.dim_if_necessary(color);
        if self.num_colors > color {
            return apply_cap(&self.ti, "setab", &[Param::Number(color as i32)], &mut self.out);
        }
        Err(Error::ColorOutOfRange)
    }

    fn attr(&mut self, attr: Attr) -> Result<()> {
        match attr {
            Attr::ForegroundColor(c) => self.fg(c),
            Attr::BackgroundColor(c) => self.bg(c),
            _ => apply_cap(&self.ti, cap_for_attr(attr), &[], &mut self.out),
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match attr {
            Attr::ForegroundColor(_) | Attr::BackgroundColor(_) => self.num_colors > 0,
            _ => {
                let cap = cap_for_attr(attr);
                self.ti.strings.get(cap).is_some()
            }
        }
    }

    fn reset(&mut self) -> Result<()> {
        reset(&self.ti, &mut self.out)
    }

    fn supports_reset(&self) -> bool {
        ["sgr0", "sgr", "op"].iter().any(|&cap| self.ti.strings.get(cap).is_some())
    }

    fn supports_color(&self) -> bool {
        self.num_colors > 0 && self.supports_reset()
    }

    fn cursor_up(&mut self) -> Result<()> {
        apply_cap(&self.ti, "cuu1", &[], &mut self.out)
    }

    fn delete_line(&mut self) -> Result<()> {
        apply_cap(&self.ti, "dl", &[], &mut self.out)
    }

    fn carriage_return(&mut self) -> Result<()> {
        apply_cap(&self.ti, "cr", &[], &mut self.out)
    }

    fn get_ref(&self) -> &T {
        &self.out
    }

    fn get_mut(&mut self) -> &mut T {
        &mut self.out
    }

    fn into_inner(self) -> T
        where Self: Sized
    {
        self.out
    }
}

impl<T: Write + Send> TerminfoTerminal<T> {
    /// Create a new TerminfoTerminal with the given TermInfo and Write.
    pub fn new_with_terminfo(out: T, terminfo: Arc<TermInfo>) -> TerminfoTerminal<T> {
        let nc = if terminfo.strings.contains_key("setaf") &&
                    terminfo.strings.contains_key("setab") {
            terminfo.numbers.get("colors").map_or(0, |&n| n)
        } else {
            0
        };

        TerminfoTerminal {
            out: out,
            ti: terminfo,
            num_colors: nc,
        }
    }

    /// Create a new TerminfoTerminal for the current environment with the given Write.
    ///
    /// Returns `None` when the terminfo cannot be found or parsed.
    pub fn new(out: T) -> Option<TerminfoTerminal<T>> {
        TermInfo::from_env().map(move |ti| {
            TerminfoTerminal::new_with_terminfo(out, Arc::new(ti))
        }).ok()
    }

    fn dim_if_necessary(&self, color: color::Color) -> color::Color {
        if color >= self.num_colors && color >= 8 && color < 16 {
            color - 8
        } else {
            color
        }
    }
}

impl<T: Write> Write for TerminfoTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.out.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn cap_for_attr(attr: Attr) -> &'static str {
    match attr {
        Attr::Bold => "bold",
        Attr::Dim => "dim",
        Attr::Italic(true) => "sitm",
        Attr::Italic(false) => "ritm",
        Attr::Underline(true) => "smul",
        Attr::Underline(false) => "rmul",
        Attr::Blink => "blink",
        Attr::Standout(true) => "smso",
        Attr::Standout(false) => "rmso",
        Attr::Reverse => "rev",
        Attr::Secure => "invis",
        Attr::ForegroundColor(_) => "setaf",
        Attr::BackgroundColor(_) => "setab",
    }
}

fn apply_cap(ti: &TermInfo, cmd: &str, params: &[Param], out: &mut io::Write) -> Result<()> {
    match ti.strings.get(cmd) {
        Some(cmd) => {
            match expand(cmd, params, &mut Variables::new()) {
                Ok(s) => {
                    try!(out.write_all(&s));
                    Ok(())
                }
                Err(e) => Err(e.into()),
            }
        }
        None => Err(Error::NotSupported),
    }
}

fn reset(ti: &TermInfo, out: &mut io::Write) -> Result<()> {
    // are there any terminals that have color/attrs and not sgr0?
    // Try falling back to sgr, then op
    let cmd = match [("sgr0", &[] as &[Param]), ("sgr", &[Param::Number(0)]), ("op", &[])]
                        .iter()
                        .filter_map(|&(cap, params)| {
                            ti.strings.get(cap).map(|c| (c, params))
                        })
                        .next() {
        Some((op, params)) => {
            match expand(op, params, &mut Variables::new()) {
                Ok(cmd) => cmd,
                Err(e) => return Err(e.into()),
            }
        }
        None => return Err(Error::NotSupported),
    };
    try!(out.write_all(&cmd));
    Ok(())
}
