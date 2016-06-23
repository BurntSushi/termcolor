use std::cmp;
use std::io;

use memchr::{memchr, memrchr};
use regex::bytes::{Regex, RegexBuilder};
use syntax;

use literals::LiteralSets;
use nonl;
use {Error, Result};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Match {
    start: usize,
    end: usize,
    line: Option<usize>,
    locations: Vec<(usize, usize)>,
}

impl Match {
    pub fn new() -> Match {
        Match::default()
    }

    /// Return the starting byte offset of the line that matched.
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Return the ending byte offset of the line that matched.
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Return the line number that this match corresponds to.
    ///
    /// Note that this is `None` if line numbers aren't being computed. Line
    /// number tracking can be enabled using `GrepBuilder`.
    #[inline]
    pub fn line(&self) -> Option<usize> {
        self.line
    }

    /// Return the exact start and end locations (in byte offsets) of every
    /// regex match in this line.
    ///
    /// Note that this always returns an empty slice if exact locations aren't
    /// computed. Exact location tracking can be enabled using `GrepBuilder`.
    #[inline]
    pub fn locations(&self) -> &[(usize, usize)] {
        &self.locations
    }
}

#[derive(Clone, Debug)]
pub struct Grep {
    re: Regex,
    required: Option<Regex>,
    opts: Options,
}

#[derive(Clone, Debug)]
pub struct GrepBuilder {
    pattern: String,
    opts: Options,
}

#[derive(Clone, Debug)]
struct Options {
    case_insensitive: bool,
    lines: bool,
    locations: bool,
    line_terminator: u8,
    size_limit: usize,
    dfa_size_limit: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            case_insensitive: false,
            lines: false,
            locations: false,
            line_terminator: b'\n',
            size_limit: 10 * (1 << 20),
            dfa_size_limit: 10 * (1 << 20),
        }
    }
}

impl GrepBuilder {
    /// Create a new builder for line searching.
    ///
    /// The pattern given should be a regular expression. The precise syntax
    /// supported is documented on the regex crate.
    pub fn new(pattern: &str) -> GrepBuilder {
        GrepBuilder {
            pattern: pattern.to_string(),
            opts: Options::default(),
        }
    }

    /// Sets whether line numbers are reported for each match.
    ///
    /// When enabled (disabled by default), every matching line is tagged with
    /// its corresponding line number accoring to the line terminator that is
    /// set. Note that this requires extra processing which can slow down
    /// search.
    pub fn line_numbers(mut self, yes: bool) -> GrepBuilder {
        self.opts.lines = yes;
        self
    }

    /// Set whether precise match locations are reported for each matching
    /// line.
    ///
    /// When enabled (disabled by default), every match of the regex on each
    /// matchling line is reported via byte offsets. Note that this requires
    /// extra processing which can slow down search.
    pub fn locations(mut self, yes: bool) -> GrepBuilder {
        self.opts.locations = yes;
        self
    }

    /// Set the line terminator.
    ///
    /// The line terminator can be any ASCII character and serves to delineate
    /// the match boundaries in the text searched.
    ///
    /// This panics if `ascii_byte` is greater than `0x7F` (i.e., not ASCII).
    pub fn line_terminator(mut self, ascii_byte: u8) -> GrepBuilder {
        assert!(ascii_byte <= 0x7F);
        self.opts.line_terminator = ascii_byte;
        self
    }

    /// Set the case sensitive flag (`i`) on the regex.
    pub fn case_insensitive(mut self, yes: bool) -> GrepBuilder {
        self.opts.case_insensitive = yes;
        self
    }

    /// Set the approximate size limit of the compiled regular expression.
    ///
    /// This roughly corresponds to the number of bytes occupied by a
    /// single compiled program. If the program exceeds this number, then a
    /// compilation error is returned.
    pub fn size_limit(mut self, limit: usize) -> GrepBuilder {
        self.opts.size_limit = limit;
        self
    }

    /// Set the approximate size of the cache used by the DFA.
    ///
    /// This roughly corresponds to the number of bytes that the DFA will use
    /// while searching.
    ///
    /// Note that this is a per thread limit. There is no way to set a global
    /// limit. In particular, if a regex is used from multiple threads
    /// simulanteously, then each thread may use up to the number of bytes
    /// specified here.
    pub fn dfa_size_limit(mut self, limit: usize) -> GrepBuilder {
        self.opts.dfa_size_limit = limit;
        self
    }

    /// Create a line searcher.
    ///
    /// If there was a problem parsing or compiling the regex with the given
    /// options, then an error is returned.
    pub fn create(self) -> Result<Grep> {
        let expr = try!(self.parse());
        let literals = LiteralSets::create(&expr);
        let re = try!(
            RegexBuilder::new(&expr.to_string())
                .case_insensitive(self.opts.case_insensitive)
                .multi_line(true)
                .unicode(true)
                .size_limit(self.opts.size_limit)
                .dfa_size_limit(self.opts.dfa_size_limit)
                .compile()
        );
        Ok(Grep {
            re: re,
            required: literals.to_regex(),
            opts: self.opts,
        })
    }

    /// Parses the underlying pattern and ensures the pattern can never match
    /// the line terminator.
    fn parse(&self) -> Result<syntax::Expr> {
        let expr =
            try!(syntax::ExprBuilder::new()
                 .allow_bytes(true)
                 .unicode(true)
                 .parse(&self.pattern));
        Ok(try!(nonl::remove(expr, self.opts.line_terminator)))
    }
}

impl Grep {
    pub fn iter<'b, 's>(&'s self, buf: &'b [u8]) -> Iter<'b, 's> {
        Iter {
            searcher: self,
            buf: buf,
            start: 0,
        }
    }

    pub fn buffered_reader<'g, R: io::Read>(
        &'g self,
        buf: Buffer,
        rdr: R,
    ) -> GrepBuffered<'g, R> {
        GrepBuffered {
            grep: self,
            rdr: rdr,
            b: buf,
            pos: 0,
            start: 0,
            lastnl: 0,
            end: 0,
        }
    }

    pub fn read_match(
        &self,
        mat: &mut Match,
        buf: &[u8],
        mut start: usize,
    ) -> bool {
        if start >= buf.len() {
            return false;
        }
        if let Some(ref req) = self.required {
            while start < buf.len() {
                let e = match req.shortest_match(&buf[start..]) {
                    None => return false,
                    Some(e) => start + e,
                };
                let (prevnl, nextnl) = self.find_line(buf, e, e);
                match self.re.shortest_match(&buf[prevnl..nextnl]) {
                    None => {
                        start = nextnl + 1;
                        continue;
                    }
                    Some(_) => {
                        self.fill_match(mat, prevnl, nextnl);
                        return true;
                    }
                }
            }
            false
        } else {
            let e = match self.re.shortest_match(&buf[start..]) {
                None => return false,
                Some(e) => start + e,
            };
            let (s, e) = self.find_line(buf, e, e);
            self.fill_match(mat, s, e);
            true
        }
    }

    fn fill_match(&self, mat: &mut Match, start: usize, end: usize) {
        mat.start = start;
        mat.end = end;
    }

    fn find_line(&self, buf: &[u8], s: usize, e: usize) -> (usize, usize) {
        (self.find_line_start(buf, s), self.find_line_end(buf, e))
    }

    fn find_line_start(&self, buf: &[u8], pos: usize) -> usize {
        memrchr(self.opts.line_terminator, &buf[0..pos]).map_or(0, |i| i + 1)
    }

    fn find_line_end(&self, buf: &[u8], pos: usize) -> usize {
        memchr(self.opts.line_terminator, &buf[pos..])
            .map_or(buf.len(), |i| pos + i)
    }
}

pub struct Buffer {
    buf: Vec<u8>,
    tmp: Vec<u8>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer::with_capacity(16 * (1<<10))
    }

    pub fn with_capacity(cap: usize) -> Buffer {
        Buffer {
            buf: vec![0; cap],
            tmp: Vec::new(),
        }
    }
}

pub struct GrepBuffered<'g, R> {
    grep: &'g Grep,
    rdr: R,
    b: Buffer,
    pos: usize,
    start: usize,
    lastnl: usize,
    end: usize,
}

impl<'g, R: io::Read> GrepBuffered<'g, R> {
    pub fn into_buffer(self) -> Buffer {
        self.b
    }

    pub fn iter<'b>(&'b mut self) -> IterBuffered<'b, 'g, R> {
        IterBuffered { grep: self }
    }

    pub fn read_match(
        &mut self,
        mat: &mut Match,
    ) -> Result<bool> {
        loop {
            if self.start == self.lastnl {
                if !try!(self.fill()) {
                    return Ok(false);
                }
            }
            let ok = self.grep.read_match(
                mat, &self.b.buf[..self.lastnl], self.start);
            if !ok {
                self.start = self.lastnl;
                continue;
            }
            // Move start to the first possible byte of the next line.
            self.start = cmp::min(
                self.lastnl, mat.end.checked_add(1).unwrap());
            mat.start += self.pos;
            mat.end += self.pos;
            return Ok(true);
        }
    }

    fn fill(&mut self) -> Result<bool> {
        {
            // The buffer might have leftover bytes that have not been
            // searched yet. Leftovers correspond to all bytes proceding the
            // final \n in the current buffer.
            //
            // TODO(ag): Seems like we should be able to memmove from the end
            // of the buffer to the beginning, but let's do it the stupid (but
            // safe) way for now.
            let leftovers = &self.b.buf[self.lastnl..self.end];
            self.b.tmp.clear();
            self.b.tmp.resize(leftovers.len(), 0);
            self.b.tmp.copy_from_slice(leftovers);
        }
        // Move the leftovers to the beginning of our buffer.
        self.b.buf[0..self.b.tmp.len()].copy_from_slice(&self.b.tmp);
        // Fill the rest with fresh bytes.
        let nread = try!(self.rdr.read(&mut self.b.buf[self.b.tmp.len()..]));
        // Now update our various positions.
        self.pos += self.start;
        println!("start: {:?}, pos: {:?}", self.start, self.pos);
        self.start = 0;
        // The end is the total number of bytes read plus whatever we had for
        // leftovers.
        self.end = self.b.tmp.len() + nread;
        // Find the last new line. All searches on this buffer will be capped
        // at this position since any proceding bytes may correspond to a
        // partial line.
        //
        // This is a little complicated because must handle the case where
        // the buffer is not full and no new line character could be found.
        // We detect this case because this could potentially be a partial
        // line. If we fill our buffer and still can't find a `\n`, then we
        // give up.
        let mut start = 0;
        let term = self.grep.opts.line_terminator;
        loop {
            match memrchr(term, &self.b.buf[start..self.end]) {
                Some(i) => {
                    self.lastnl = start + i + 1;
                    break;
                }
                None => {
                    // If we couldn't find a new line and our buffer is
                    // completely full, then this line is terribly long and we
                    // return an error.
                    if self.end == self.b.buf.len() {
                        return Err(Error::LineTooLong(self.b.buf.len()));
                    }
                    // Otherwise we try to ask for more bytes and look again.
                    let nread = try!(
                        self.rdr.read(&mut self.b.buf[self.end..]));
                    // If we got nothing than we're at EOF and we no longer
                    // need to care about leftovers.
                    if nread == 0 {
                        self.lastnl = self.end;
                        break;
                    }
                    start = self.end;
                    self.end += nread;
                }
            }
        }
        // If end is zero, then we've hit EOF and we have no leftovers.
        Ok(self.end > 0)
    }
}

pub struct Iter<'b, 's> {
    searcher: &'s Grep,
    buf: &'b [u8],
    start: usize,
}

impl<'b, 's> Iterator for Iter<'b, 's> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        let mut mat = Match::default();
        if !self.searcher.read_match(&mut mat, self.buf, self.start) {
            self.start = self.buf.len();
            return None;
        }
        self.start = mat.end + 1;
        Some(mat)
    }
}

pub struct IterBuffered<'b, 'g: 'b, R: 'b> {
    grep: &'b mut GrepBuffered<'g, R>,
}

impl<'b, 'g, R: io::Read> Iterator for IterBuffered<'b, 'g, R> {
    type Item = Result<Match>;

    fn next(&mut self) -> Option<Result<Match>> {
        let mut mat = Match::default();
        match self.grep.read_match(&mut mat) {
            Err(err) => Some(Err(err)),
            Ok(false) => None,
            Ok(true) => Some(Ok(mat)),
        }
    }
}

#[allow(dead_code)]
fn s(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::{Buffer, GrepBuilder, s};

    static SHERLOCK: &'static [u8] = include_bytes!("./data/sherlock.txt");

    #[test]
    fn buffered() {
        let g = GrepBuilder::new("Sherlock Holmes").create().unwrap();
        let mut bg = g.buffered_reader(Buffer::new(), SHERLOCK);
        let ms: Vec<_> = bg.iter().map(|r| r.unwrap()).collect();
        let m = ms.last().unwrap();
        assert_eq!(91, ms.len());
        assert_eq!(575707, m.start());
        assert_eq!(575784, m.end());
    }
}
