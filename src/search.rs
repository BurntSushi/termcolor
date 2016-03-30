use memchr::{memchr, memrchr};
use regex::bytes::Regex;
use syntax;

use literals::LiteralSets;
use nonl;
use Result;

#[derive(Clone, Debug)]
pub struct LineSearcher {
    re: Regex,
    required: Option<Regex>,
    opts: Options,
}

#[derive(Clone, Debug)]
pub struct LineSearcherBuilder {
    pattern: String,
    opts: Options,
}

#[derive(Clone, Debug, Default)]
struct Options {
    case_insensitive: bool,
    lines: bool,
    locations: bool,
}

impl LineSearcherBuilder {
    pub fn new(pattern: &str) -> LineSearcherBuilder {
        LineSearcherBuilder {
            pattern: pattern.to_string(),
            opts: Options::default(),
        }
    }

    pub fn case_insensitive(mut self, yes: bool) -> LineSearcherBuilder {
        self.opts.case_insensitive = yes;
        self
    }

    pub fn line_numbers(mut self, yes: bool) -> LineSearcherBuilder {
        self.opts.lines = yes;
        self
    }

    pub fn locations(mut self, yes: bool) -> LineSearcherBuilder {
        self.opts.locations = yes;
        self
    }

    pub fn create(self) -> Result<LineSearcher> {
        let expr = try!(parse(&self.pattern));
        let literals = LiteralSets::create(&expr);
        let pat =
            if self.opts.case_insensitive {
                format!("(?i){}", expr)
            } else {
                expr.to_string()
            };
        // We've already parsed the pattern, so we know it will compiled.
        let re = Regex::new(&pat).unwrap();
        Ok(LineSearcher {
            re: re,
            required: literals.to_matcher(),
            opts: self.opts,
        })
    }
}

impl LineSearcher {
    pub fn search<'b, 's>(&'s self, buf: &'b [u8]) -> Iter<'b, 's> {
        Iter {
            searcher: self,
            buf: buf,
            start: 0,
            count: 0,
        }
    }
}

pub struct Match {
    pub start: usize,
    pub end: usize,
    pub count: usize,
    pub line: Option<usize>,
    pub locations: Vec<(usize, usize)>,
}

pub struct Iter<'b, 's> {
    searcher: &'s LineSearcher,
    buf: &'b [u8],
    start: usize,
    count: usize,
}

impl<'b, 's> Iter<'b, 's> {
    fn next_line_match(&mut self) -> Option<(usize, usize)> {
        if self.start >= self.buf.len() {
            return None;
        }
        if let Some(ref req) = self.searcher.required {
            while self.start < self.buf.len() {
                let (s, e) = match req.find(&self.buf[self.start..]) {
                    None => return None,
                    Some((s, e)) => (self.start + s, self.start + e),
                };
                let (prevnl, nextnl) = self.find_line(s, e);
                match self.searcher.re.find(&self.buf[prevnl..nextnl]) {
                    None => {
                        self.start = nextnl + 1;
                        continue;
                    }
                    Some(_) => return Some((prevnl, nextnl)),
                }
            }
            None
        } else {
            let (s, e) = match self.searcher.re.find(&self.buf[self.start..]) {
                None => return None,
                Some((s, e)) => (self.start + s, self.start + e),
            };
            Some(self.find_line(s, e))
        }
    }

    fn find_line(&self, s: usize, e: usize) -> (usize, usize) {
        let prevnl =
            memrchr(b'\n', &self.buf[0..s]).map_or(0, |i| i + 1);
        let nextnl =
            memchr(b'\n', &self.buf[e..]).map_or(self.buf.len(), |i| e + i);
        (prevnl, nextnl)
    }
}

impl<'b, 's> Iterator for Iter<'b, 's> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        let (prevnl, nextnl) = match self.next_line_match() {
            None => return None,
            Some((s, e)) => (s, e),
        };
        let count = self.count;
        self.start = nextnl + 1;
        self.count += 1;
        Some(Match {
            start: prevnl,
            end: nextnl,
            count: count,
            line: None,
            locations: vec![],
        })
    }
}

fn parse(re: &str) -> Result<syntax::Expr> {
    let expr =
        try!(syntax::ExprBuilder::new()
             .allow_bytes(true)
             .unicode(false)
             .parse(re));
    Ok(try!(nonl::remove(expr)))
}
