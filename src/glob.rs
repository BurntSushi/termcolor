/*!
The glob module provides standard shell globbing, but is specifically
implemented by converting glob syntax to regular expressions. The reasoning is
two fold:

1. The regex library is *really* fast. Regaining performance in a distinct
   implementation of globbing is non-trivial.
2. Most crucially, a `RegexSet` can be used to match many globs simultaneously.

This module is written with some amount of intention of eventually splitting it
out into its own separate crate, but I didn't quite have the energy for all
that rigamorole when I wrote this. In particular, it could be fast/good enough
to make its way into `glob` proper.
*/

// TODO(burntsushi): I'm pretty dismayed by the performance of regex sets
// here. For example, we do a first pass single-regex-of-all-globs filter
// before actually running the regex set. This turns out to be faster,
// especially in fresh checkouts of repos that don't have a lot of ignored
// files. It's not clear how hard it is to make the regex set faster.
//
// An alternative avenue is to stop doing "regex all the things." (Which, to
// be fair, is pretty fast---I just expected it to be faster.) We could do
// something clever using assumptions along the lines of "oh, most ignore
// patterns are either literals or are for ignoring file extensions." (Look
// at the .gitignore for the chromium repo---just about every pattern satisfies
// that assumption.)

use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::hash;
use std::iter;
use std::path::Path;
use std::str;

use fnv;
use regex;
use regex::bytes::Regex;
use regex::bytes::RegexSet;

use pathutil::file_name;

/// Represents an error that can occur when parsing a glob pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidRecursive,
    UnclosedClass,
    InvalidRange(char, char),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidRecursive => {
                "invalid use of **; must be one path component"
            }
            Error::UnclosedClass => {
                "unclosed character class; missing ']'"
            }
            Error::InvalidRange(_, _) => {
                "invalid character range"
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::InvalidRecursive | Error::UnclosedClass => {
                write!(f, "{}", self.description())
            }
            Error::InvalidRange(s, e) => {
                write!(f, "invalid range; '{}' > '{}'", s, e)
            }
        }
    }
}

/// SetYesNo represents a group of globs that can be matched together in a
/// single pass. SetYesNo can only determine whether a particular path matched
/// any pattern in the set.
#[derive(Clone, Debug)]
pub struct SetYesNo {
    re: Regex,
}

impl SetYesNo {
    /// Returns true if and only if the given path matches at least one glob
    /// in this set.
    pub fn is_match<T: AsRef<Path>>(&self, path: T) -> bool {
        self.re.is_match(&*path_bytes(path.as_ref()))
    }

    fn new(
        pats: &[(Pattern, MatchOptions)],
    ) -> Result<SetYesNo, regex::Error> {
        let mut joined = String::new();
        for &(ref p, ref o) in pats {
            let part = format!("(?:{})", p.to_regex_with(o));
            if !joined.is_empty() {
                joined.push('|');
            }
            joined.push_str(&part);
        }
        Ok(SetYesNo { re: try!(Regex::new(&joined)) })
    }
}

type Fnv = hash::BuildHasherDefault<fnv::FnvHasher>;

/// Set represents a group of globs that can be matched together in a single
/// pass.
#[derive(Clone, Debug)]
pub struct Set {
    yesno: SetYesNo,
    exts: HashMap<OsString, Vec<usize>, Fnv>,
    literals: HashMap<Vec<u8>, Vec<usize>, Fnv>,
    base_literals: HashMap<Vec<u8>, Vec<usize>, Fnv>,
    base_prefixes: Vec<Vec<u8>>,
    base_prefixes_map: Vec<usize>,
    base_suffixes: Vec<Vec<u8>>,
    base_suffixes_map: Vec<usize>,
    regexes: RegexSet,
    regexes_map: Vec<usize>,
}

impl Set {
    /// Returns the sequence number of every glob pattern that matches the
    /// given path.
    #[allow(dead_code)]
    pub fn matches<T: AsRef<Path>>(&self, path: T) -> Vec<usize> {
        let mut into = vec![];
        self.matches_into(path, &mut into);
        into
    }

    /// Adds the sequence number of every glob pattern that matches the given
    /// path to the vec given.
    pub fn matches_into<T: AsRef<Path>>(
        &self,
        path: T,
        into: &mut Vec<usize>,
    ) {
        into.clear();
        let path = path.as_ref();
        let path_bytes = &*path_bytes(path);
        let basename = file_name(path).map(|b| os_str_bytes(b));
        if !self.yesno.is_match(path) {
            return;
        }
        if !self.exts.is_empty() {
            if let Some(ext) = path.extension() {
                if let Some(matches) = self.exts.get(ext) {
                    into.extend(matches.as_slice());
                }
            }
        }
        if !self.literals.is_empty() {
            if let Some(matches) = self.literals.get(path_bytes) {
                into.extend(matches.as_slice());
            }
        }
        if !self.base_literals.is_empty() {
            if let Some(ref basename) = basename {
                if let Some(matches) = self.base_literals.get(&**basename) {
                    into.extend(matches.as_slice());
                }
            }
        }
        if !self.base_prefixes.is_empty() {
            if let Some(ref basename) = basename {
                let basename = &**basename;
                for (i, pre) in self.base_prefixes.iter().enumerate() {
                    if pre.len() <= basename.len() && &**pre == &basename[0..pre.len()] {
                        into.push(self.base_prefixes_map[i]);
                    }
                }
            }
        }
        if !self.base_suffixes.is_empty() {
            if let Some(ref basename) = basename {
                let basename = &**basename;
                for (i, suf) in self.base_suffixes.iter().enumerate() {
                    if suf.len() > basename.len() {
                        continue;
                    }
                    let (s, e) = (basename.len() - suf.len(), basename.len());
                    if &**suf == &basename[s..e] {
                        into.push(self.base_suffixes_map[i]);
                    }
                }
            }
        }
        into.extend(self.regexes.matches(path_bytes));
        into.sort();
    }

    fn new(pats: &[(Pattern, MatchOptions)]) -> Result<Set, regex::Error> {
        let fnv = Fnv::default();
        let mut exts = HashMap::with_hasher(fnv.clone());
        let mut literals = HashMap::with_hasher(fnv.clone());
        let mut base_literals = HashMap::with_hasher(fnv.clone());
        let (mut base_prefixes, mut base_prefixes_map) = (vec![], vec![]);
        let (mut base_suffixes, mut base_suffixes_map) = (vec![], vec![]);
        let (mut regexes, mut regexes_map) = (vec![], vec![]);
        for (i, &(ref p, ref o)) in pats.iter().enumerate() {
            if let Some(ext) = p.ext() {
                exts.entry(ext).or_insert(vec![]).push(i);
            } else if let Some(literal) = p.literal() {
                literals.entry(literal.into_bytes()).or_insert(vec![]).push(i);
            } else if let Some(literal) = p.base_literal() {
                base_literals
                    .entry(literal.into_bytes()).or_insert(vec![]).push(i);
            } else if let Some(literal) = p.base_literal_prefix() {
                base_prefixes.push(literal.into_bytes());
                base_prefixes_map.push(i);
            } else if let Some(literal) = p.base_literal_suffix() {
                base_suffixes.push(literal.into_bytes());
                base_suffixes_map.push(i);
            } else {
                let part = format!("(?:{})", p.to_regex_with(o));
                regexes.push(part);
                regexes_map.push(i);
            }
        }
        Ok(Set {
            yesno: try!(SetYesNo::new(pats)),
            exts: exts,
            literals: literals,
            base_literals: base_literals,
            base_prefixes: base_prefixes,
            base_prefixes_map: base_prefixes_map,
            base_suffixes: base_suffixes,
            base_suffixes_map: base_suffixes_map,
            regexes: try!(RegexSet::new(regexes)),
            regexes_map: regexes_map,
        })
    }
}

/// SetBuilder builds a group of patterns that can be used to simultaneously
/// match a file path.
pub struct SetBuilder {
    pats: Vec<(Pattern, MatchOptions)>,
}

impl SetBuilder {
    /// Create a new SetBuilder. A SetBuilder can be used to add new patterns.
    /// Once all patterns have been added, `build` should be called to produce
    /// a `Set`, which can then be used for matching.
    pub fn new() -> SetBuilder {
        SetBuilder { pats: vec![] }
    }

    /// Builds a new matcher from all of the glob patterns added so far.
    ///
    /// Once a matcher is built, no new patterns can be added to it.
    pub fn build(&self) -> Result<Set, regex::Error> {
        Set::new(&self.pats)
    }

    /// Like `build`, but returns a matcher that can only answer yes/no.
    pub fn build_yesno(&self) -> Result<SetYesNo, regex::Error> {
        SetYesNo::new(&self.pats)
    }

    /// Add a new pattern to this set.
    ///
    /// If the pattern could not be parsed as a glob, then an error is
    /// returned.
    #[allow(dead_code)]
    pub fn add(&mut self, pat: &str) -> Result<(), Error> {
        self.add_with(pat, &MatchOptions::default())
    }

    /// Like add, but sets the match options for this particular pattern.
    pub fn add_with(
        &mut self,
        pat: &str,
        opts: &MatchOptions,
    ) -> Result<(), Error> {
        let parsed = try!(Pattern::new(pat));
        // if let Some(ext) = parsed.ext() {
            // eprintln!("ext :: {:?} :: {:?}", ext, pat);
        // } else if let Some(lit) = parsed.literal() {
            // eprintln!("literal :: {:?} :: {:?}", lit, pat);
        // } else if let Some(lit) = parsed.base_literal() {
            // eprintln!("base_literal :: {:?} :: {:?}", lit, pat);
        // } else if let Some(lit) = parsed.base_literal_prefix() {
            // eprintln!("base_literal :: {:?} :: {:?}", lit, pat);
        // } else if let Some(lit) = parsed.base_literal_suffix() {
            // eprintln!("base_literal :: {:?} :: {:?}", lit, pat);
        // } else {
            // eprintln!("regex :: {:?} :: {:?}", pat, parsed);
        // }
        self.pats.push((parsed, opts.clone()));
        Ok(())
    }
}

/// Pattern represents a successfully parsed shell glob pattern.
///
/// It cannot be used directly to match file paths, but it can be converted
/// to a regular expression string.
#[derive(Clone, Debug, Default)]
pub struct Pattern {
    tokens: Vec<Token>,
}

/// Options to control the matching semantics of a glob. The default value
/// has all options disabled.
#[derive(Clone, Debug, Default)]
pub struct MatchOptions {
    /// When true, matching is done case insensitively.
    pub case_insensitive: bool,
    /// When true, neither `*` nor `?` match the current system's path
    /// separator.
    pub require_literal_separator: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Literal(char),
    Any,
    ZeroOrMore,
    RecursivePrefix,
    RecursiveSuffix,
    RecursiveZeroOrMore,
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl Pattern {
    /// Parse a shell glob pattern.
    ///
    /// If the pattern is not a valid glob, then an error is returned.
    pub fn new(pat: &str) -> Result<Pattern, Error> {
        let mut p = Parser {
            p: Pattern::default(),
            chars: pat.chars().peekable(),
            prev: None,
            cur: None,
        };
        try!(p.parse());
        Ok(p.p)
    }

    /// Returns an extension if this pattern exclusively matches it.
    pub fn ext(&self) -> Option<OsString> {
        if self.tokens.len() <= 3 {
            return None;
        }
        match self.tokens.get(0) {
            Some(&Token::RecursivePrefix) => {}
            _ => return None,
        }
        match self.tokens.get(1) {
            Some(&Token::ZeroOrMore) => {}
            _ => return None,
        }
        match self.tokens.get(2) {
            Some(&Token::Literal(c)) if c == '.' => {}
            _ => return None,
        }
        let mut lit = OsString::new();
        for t in self.tokens[3..].iter() {
            match *t {
                Token::Literal(c) if c == '/' || c == '\\' || c == '.' => {
                    return None;
                }
                Token::Literal(c) => lit.push(c.to_string()),
                _ => return None,
            }
        }
        Some(lit)
    }

    /// Returns the pattern as a literal if and only if the pattern exclusiely
    /// matches the basename of a file path *and* is a literal.
    ///
    /// The basic format of these patterns is `**/{literal}`, where `{literal}`
    /// does not contain a path separator.
    pub fn base_literal(&self) -> Option<String> {
        match self.tokens.get(0) {
            Some(&Token::RecursivePrefix) => {}
            _ => return None,
        }
        let mut lit = String::new();
        for t in &self.tokens[1..] {
            match *t {
                Token::Literal(c) if c == '/' || c == '\\' => return None,
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        Some(lit)
    }

    /// Returns the pattern as a literal if and only if the pattern must match
    /// an entire path exactly.
    ///
    /// The basic format of these patterns is `{literal}`.
    pub fn literal(&self) -> Option<String> {
        let mut lit = String::new();
        for t in &self.tokens {
            match *t {
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        Some(lit)
    }

    /// Returns a basename literal prefix of this pattern.
    pub fn base_literal_prefix(&self) -> Option<String> {
        match self.tokens.get(0) {
            Some(&Token::RecursivePrefix) => {}
            _ => return None,
        }
        match self.tokens.last() {
            Some(&Token::ZeroOrMore) => {}
            _ => return None,
        }
        let mut lit = String::new();
        for t in &self.tokens[1..self.tokens.len()-1] {
            match *t {
                Token::Literal(c) if c == '/' || c == '\\' => return None,
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        Some(lit)
    }

    /// Returns a basename literal suffix of this pattern.
    pub fn base_literal_suffix(&self) -> Option<String> {
        match self.tokens.get(0) {
            Some(&Token::RecursivePrefix) => {}
            _ => return None,
        }
        match self.tokens.get(1) {
            Some(&Token::ZeroOrMore) => {}
            _ => return None,
        }
        let mut lit = String::new();
        for t in &self.tokens[2..] {
            match *t {
                Token::Literal(c) if c == '/' || c == '\\' => return None,
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        Some(lit)
    }

    /// Convert this pattern to a string that is guaranteed to be a valid
    /// regular expression and will represent the matching semantics of this
    /// glob pattern. This uses a default set of options.
    #[allow(dead_code)]
    pub fn to_regex(&self) -> String {
        self.to_regex_with(&MatchOptions::default())
    }

    /// Convert this pattern to a string that is guaranteed to be a valid
    /// regular expression and will represent the matching semantics of this
    /// glob pattern and the options given.
    pub fn to_regex_with(&self, options: &MatchOptions) -> String {
        let seps = regex::quote(r"/\");
        let mut re = String::new();
        re.push_str("(?-u)");
        if options.case_insensitive {
            re.push_str("(?i)");
        }
        re.push('^');
        // Special case. If the entire glob is just `**`, then it should match
        // everything.
        if self.tokens.len() == 1 && self.tokens[0] == Token::RecursivePrefix {
            re.push_str(".*");
            re.push('$');
            return re;
        }
        for tok in &self.tokens {
            match *tok {
                Token::Literal(c) => {
                    re.push_str(&regex::quote(&c.to_string()));
                }
                Token::Any => {
                    if options.require_literal_separator {
                        re.push_str(&format!("[^{}]", seps));
                    } else {
                        re.push_str(".");
                    }
                }
                Token::ZeroOrMore => {
                    if options.require_literal_separator {
                        re.push_str(&format!("[^{}]*", seps));
                    } else {
                        re.push_str(".*");
                    }
                }
                Token::RecursivePrefix => {
                    re.push_str(&format!("(?:[{sep}]?|.*[{sep}])", sep=seps));
                }
                Token::RecursiveSuffix => {
                    re.push_str(&format!("(?:[{sep}]?|[{sep}].*)", sep=seps));
                }
                Token::RecursiveZeroOrMore => {
                    re.push_str(&format!("(?:[{sep}]|[{sep}].*[{sep}])",
                                         sep=seps));
                }
                Token::Class { negated, ref ranges } => {
                    re.push('[');
                    if negated {
                        re.push('^');
                    }
                    for r in ranges {
                        if r.0 == r.1 {
                            // Not strictly necessary, but nicer to look at.
                            re.push_str(&regex::quote(&r.0.to_string()));
                        } else {
                            re.push_str(&regex::quote(&r.0.to_string()));
                            re.push('-');
                            re.push_str(&regex::quote(&r.1.to_string()));
                        }
                    }
                    re.push(']');
                }
            }
        }
        re.push('$');
        re
    }
}

struct Parser<'a> {
    p: Pattern,
    chars: iter::Peekable<str::Chars<'a>>,
    prev: Option<char>,
    cur: Option<char>,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<(), Error> {
        while let Some(c) = self.bump() {
            match c {
                '?' => self.p.tokens.push(Token::Any),
                '*' => try!(self.parse_star()),
                '[' => try!(self.parse_class()),
                c => self.p.tokens.push(Token::Literal(c)),
            }
        }
        Ok(())
    }

    fn parse_star(&mut self) -> Result<(), Error> {
        let prev = self.prev;
        if self.chars.peek() != Some(&'*') {
            self.p.tokens.push(Token::ZeroOrMore);
            return Ok(());
        }
        assert!(self.bump() == Some('*'));
        if self.p.tokens.is_empty() {
            self.p.tokens.push(Token::RecursivePrefix);
            let next = self.bump();
            if !next.is_none() && next != Some('/') {
                return Err(Error::InvalidRecursive);
            }
            return Ok(());
        }
        self.p.tokens.pop().unwrap();
        if prev != Some('/') {
            return Err(Error::InvalidRecursive);
        }
        let next = self.bump();
        if next.is_none() {
            self.p.tokens.push(Token::RecursiveSuffix);
            return Ok(());
        }
        if next != Some('/') {
            return Err(Error::InvalidRecursive);
        }
        self.p.tokens.push(Token::RecursiveZeroOrMore);
        Ok(())
    }

    fn parse_class(&mut self) -> Result<(), Error> {
        fn add_to_last_range(
            r: &mut (char, char),
            add: char,
        ) -> Result<(), Error> {
            r.1 = add;
            if r.1 < r.0 {
                Err(Error::InvalidRange(r.0, r.1))
            } else {
                Ok(())
            }
        }
        let mut negated = false;
        let mut ranges = vec![];
        if self.chars.peek() == Some(&'!') {
            assert!(self.bump() == Some('!'));
            negated = true;
        }
        let mut first = true;
        let mut in_range = false;
        loop {
            let c = match self.bump() {
                Some(c) => c,
                // The only way to successfully break this loop is to observe
                // a ']'.
                None => return Err(Error::UnclosedClass),
            };
            match c {
                ']' => {
                    if first {
                        ranges.push((']', ']'));
                    } else {
                        break;
                    }
                }
                '-' => {
                    if first {
                        ranges.push(('-', '-'));
                    } else if in_range {
                        // invariant: in_range is only set when there is
                        // already at least one character seen.
                        let r = ranges.last_mut().unwrap();
                        try!(add_to_last_range(r, '-'));
                        in_range = false;
                    } else {
                        assert!(!ranges.is_empty());
                        in_range = true;
                    }
                }
                c => {
                    if in_range {
                        // invariant: in_range is only set when there is
                        // already at least one character seen.
                        try!(add_to_last_range(ranges.last_mut().unwrap(), c));
                    } else {
                        ranges.push((c, c));
                    }
                    in_range = false;
                }
            }
            first = false;
        }
        if in_range {
            // Means that the last character in the class was a '-', so add
            // it as a literal.
            ranges.push(('-', '-'));
        }
        self.p.tokens.push(Token::Class {
            negated: negated,
            ranges: ranges,
        });
        Ok(())
    }

    fn bump(&mut self) -> Option<char> {
        self.prev = self.cur;
        self.cur = self.chars.next();
        self.cur
    }
}

fn path_bytes(path: &Path) -> Cow<[u8]> {
    os_str_bytes(path.as_os_str())
}

#[cfg(unix)]
fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(s.as_bytes())
}

#[cfg(not(unix))]
fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    // TODO(burntsushi): On Windows, OS strings are probably UTF-16, so even
    // if we could get at the raw bytes, they wouldn't be useful. We *must*
    // convert to UTF-8 before doing path matching. Unfortunate, but necessary.
    match s.to_string_lossy() {
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use regex::bytes::Regex;

    use super::{Error, Pattern, MatchOptions, Set, SetBuilder, Token};
    use super::Token::*;

    macro_rules! syntax {
        ($name:ident, $pat:expr, $tokens:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                assert_eq!($tokens, pat.tokens);
            }
        }
    }

    macro_rules! syntaxerr {
        ($name:ident, $pat:expr, $err:expr) => {
            #[test]
            fn $name() {
                let err = Pattern::new($pat).unwrap_err();
                assert_eq!($err, err);
            }
        }
    }

    macro_rules! toregex {
        ($name:ident, $pat:expr, $re:expr) => {
            toregex!($name, $pat, $re, MatchOptions::default());
        };
        ($name:ident, $pat:expr, $re:expr, $options:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                assert_eq!(
                    format!("(?-u){}", $re), pat.to_regex_with(&$options));
            }
        };
    }

    macro_rules! matches {
        ($name:ident, $pat:expr, $path:expr) => {
            matches!($name, $pat, $path, MatchOptions::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                let path = &Path::new($path).to_str().unwrap();
                let re = Regex::new(&pat.to_regex_with(&$options)).unwrap();
                assert!(re.is_match(path.as_bytes()));
            }
        };
    }

    macro_rules! nmatches {
        ($name:ident, $pat:expr, $path:expr) => {
            nmatches!($name, $pat, $path, MatchOptions::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                let path = &Path::new($path).to_str().unwrap();
                let re = Regex::new(&pat.to_regex_with(&$options)).unwrap();
                assert!(!re.is_match(path.as_bytes()));
            }
        };
    }

    macro_rules! ext {
        ($name:ident, $pat:expr, $ext:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                let ext = pat.ext().map(|e| e.to_string_lossy().into_owned());
                assert_eq!($ext, ext.as_ref().map(|s| &**s));
            }
        };
    }

    macro_rules! baseliteral {
        ($name:ident, $pat:expr, $yes:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                assert_eq!($yes, pat.base_literal().is_some());
            }
        };
    }

    macro_rules! basesuffix {
        ($name:ident, $pat:expr, $yes:expr) => {
            #[test]
            fn $name() {
                let pat = Pattern::new($pat).unwrap();
                assert_eq!($yes, pat.is_literal_suffix());
            }
        };
    }

    fn class(s: char, e: char) -> Token {
        Class { negated: false, ranges: vec![(s, e)] }
    }

    fn classn(s: char, e: char) -> Token {
        Class { negated: true, ranges: vec![(s, e)] }
    }

    fn rclass(ranges: &[(char, char)]) -> Token {
        Class { negated: false, ranges: ranges.to_vec() }
    }

    fn rclassn(ranges: &[(char, char)]) -> Token {
        Class { negated: true, ranges: ranges.to_vec() }
    }

    syntax!(literal1, "a", vec![Literal('a')]);
    syntax!(literal2, "ab", vec![Literal('a'), Literal('b')]);
    syntax!(any1, "?", vec![Any]);
    syntax!(any2, "a?b", vec![Literal('a'), Any, Literal('b')]);
    syntax!(seq1, "*", vec![ZeroOrMore]);
    syntax!(seq2, "a*b", vec![Literal('a'), ZeroOrMore, Literal('b')]);
    syntax!(seq3, "*a*b*", vec![
        ZeroOrMore, Literal('a'), ZeroOrMore, Literal('b'), ZeroOrMore,
    ]);
    syntax!(rseq1, "**", vec![RecursivePrefix]);
    syntax!(rseq2, "**/", vec![RecursivePrefix]);
    syntax!(rseq3, "/**", vec![RecursiveSuffix]);
    syntax!(rseq4, "/**/", vec![RecursiveZeroOrMore]);
    syntax!(rseq5, "a/**/b", vec![
        Literal('a'), RecursiveZeroOrMore, Literal('b'),
    ]);
    syntax!(cls1, "[a]", vec![class('a', 'a')]);
    syntax!(cls2, "[!a]", vec![classn('a', 'a')]);
    syntax!(cls3, "[a-z]", vec![class('a', 'z')]);
    syntax!(cls4, "[!a-z]", vec![classn('a', 'z')]);
    syntax!(cls5, "[-]", vec![class('-', '-')]);
    syntax!(cls6, "[]]", vec![class(']', ']')]);
    syntax!(cls7, "[*]", vec![class('*', '*')]);
    syntax!(cls8, "[!!]", vec![classn('!', '!')]);
    syntax!(cls9, "[a-]", vec![rclass(&[('a', 'a'), ('-', '-')])]);
    syntax!(cls10, "[-a-z]", vec![rclass(&[('-', '-'), ('a', 'z')])]);
    syntax!(cls11, "[a-z-]", vec![rclass(&[('a', 'z'), ('-', '-')])]);
    syntax!(cls12, "[-a-z-]", vec![
        rclass(&[('-', '-'), ('a', 'z'), ('-', '-')]),
    ]);
    syntax!(cls13, "[]-z]", vec![class(']', 'z')]);
    syntax!(cls14, "[--z]", vec![class('-', 'z')]);
    syntax!(cls15, "[ --]", vec![class(' ', '-')]);
    syntax!(cls16, "[0-9a-z]", vec![rclass(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls17, "[a-z0-9]", vec![rclass(&[('a', 'z'), ('0', '9')])]);
    syntax!(cls18, "[!0-9a-z]", vec![rclassn(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls19, "[!a-z0-9]", vec![rclassn(&[('a', 'z'), ('0', '9')])]);

    syntaxerr!(err_rseq1, "a**", Error::InvalidRecursive);
    syntaxerr!(err_rseq2, "**a", Error::InvalidRecursive);
    syntaxerr!(err_rseq3, "a**b", Error::InvalidRecursive);
    syntaxerr!(err_rseq4, "***", Error::InvalidRecursive);
    syntaxerr!(err_rseq5, "/a**", Error::InvalidRecursive);
    syntaxerr!(err_rseq6, "/**a", Error::InvalidRecursive);
    syntaxerr!(err_rseq7, "/a**b", Error::InvalidRecursive);
    syntaxerr!(err_unclosed1, "[", Error::UnclosedClass);
    syntaxerr!(err_unclosed2, "[]", Error::UnclosedClass);
    syntaxerr!(err_unclosed3, "[!", Error::UnclosedClass);
    syntaxerr!(err_unclosed4, "[!]", Error::UnclosedClass);
    syntaxerr!(err_range1, "[z-a]", Error::InvalidRange('z', 'a'));
    syntaxerr!(err_range2, "[z--]", Error::InvalidRange('z', '-'));

    const SLASHLIT: MatchOptions = MatchOptions {
        case_insensitive: false,
        require_literal_separator: true,
    };
    const CASEI: MatchOptions = MatchOptions {
        case_insensitive: true,
        require_literal_separator: false,
    };

    toregex!(re_casei, "a", "(?i)^a$", &CASEI);

    toregex!(re_slash1, "?", r"^[^/\\]$", SLASHLIT);
    toregex!(re_slash2, "*", r"^[^/\\]*$", SLASHLIT);

    toregex!(re1, "a", "^a$");
    toregex!(re2, "?", "^.$");
    toregex!(re3, "*", "^.*$");
    toregex!(re4, "a?", "^a.$");
    toregex!(re5, "?a", "^.a$");
    toregex!(re6, "a*", "^a.*$");
    toregex!(re7, "*a", "^.*a$");
    toregex!(re8, "[*]", r"^[\*]$");
    toregex!(re9, "[+]", r"^[\+]$");
    toregex!(re10, "+", r"^\+$");
    toregex!(re11, "**", r"^.*$");

    ext!(ext1, "**/*.rs", Some("rs"));

    baseliteral!(lit1, "**", true);
    baseliteral!(lit2, "**/a", true);
    baseliteral!(lit3, "**/ab", true);
    baseliteral!(lit4, "**/a*b", false);
    baseliteral!(lit5, "z/**/a*b", false);
    baseliteral!(lit6, "[ab]", false);
    baseliteral!(lit7, "?", false);

    /*
    issuffix!(suf1, "", false);
    issuffix!(suf2, "a", true);
    issuffix!(suf3, "ab", true);
    issuffix!(suf4, "*ab", true);
    issuffix!(suf5, "*.ab", true);
    issuffix!(suf6, "?.ab", true);
    issuffix!(suf7, "ab*", false);
    */

    matches!(match1, "a", "a");
    matches!(match2, "a*b", "a_b");
    matches!(match3, "a*b*c", "abc");
    matches!(match4, "a*b*c", "a_b_c");
    matches!(match5, "a*b*c", "a___b___c");
    matches!(match6, "abc*abc*abc", "abcabcabcabcabcabcabc");
    matches!(match7, "a*a*a*a*a*a*a*a*a", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    matches!(match8, "a*b[xyz]c*d", "abxcdbxcddd");

    matches!(matchrec1, "some/**/needle.txt", "some/needle.txt");
    matches!(matchrec2, "some/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec3, "some/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec4, "some/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec5, "**", "abcde");
    matches!(matchrec6, "**", "");
    matches!(matchrec7, "**", ".asdf");
    matches!(matchrec8, "**", "/x/.asdf");
    matches!(matchrec9, "some/**/**/needle.txt", "some/needle.txt");
    matches!(matchrec10, "some/**/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec11, "some/**/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec12, "some/**/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec13, "**/test", "one/two/test");
    matches!(matchrec14, "**/test", "one/test");
    matches!(matchrec15, "**/test", "test");
    matches!(matchrec16, "/**/test", "/one/two/test");
    matches!(matchrec17, "/**/test", "/one/test");
    matches!(matchrec18, "/**/test", "/test");
    matches!(matchrec19, "**/.*", ".abc");
    matches!(matchrec20, "**/.*", "abc/.abc");
    matches!(matchrec21, ".*/**", ".abc");
    matches!(matchrec22, ".*/**", ".abc/abc");

    matches!(matchrange1, "a[0-9]b", "a0b");
    matches!(matchrange2, "a[0-9]b", "a9b");
    matches!(matchrange3, "a[!0-9]b", "a_b");
    matches!(matchrange4, "[a-z123]", "1");
    matches!(matchrange5, "[1a-z23]", "1");
    matches!(matchrange6, "[123a-z]", "1");
    matches!(matchrange7, "[abc-]", "-");
    matches!(matchrange8, "[-abc]", "-");
    matches!(matchrange9, "[-a-c]", "b");
    matches!(matchrange10, "[a-c-]", "b");
    matches!(matchrange11, "[-]", "-");

    matches!(matchpat1, "*hello.txt", "hello.txt");
    matches!(matchpat2, "*hello.txt", "gareth_says_hello.txt");
    matches!(matchpat3, "*hello.txt", "some/path/to/hello.txt");
    matches!(matchpat4, "*hello.txt", "some\\path\\to\\hello.txt");
    matches!(matchpat5, "*hello.txt", "/an/absolute/path/to/hello.txt");
    matches!(matchpat6, "*some/path/to/hello.txt", "some/path/to/hello.txt");
    matches!(matchpat7, "*some/path/to/hello.txt",
             "a/bigger/some/path/to/hello.txt");

    matches!(matchescape, "_[[]_[]]_[?]_[*]_!_", "_[_]_?_*_!_");

    matches!(matchcasei1, "aBcDeFg", "aBcDeFg", CASEI);
    matches!(matchcasei2, "aBcDeFg", "abcdefg", CASEI);
    matches!(matchcasei3, "aBcDeFg", "ABCDEFG", CASEI);
    matches!(matchcasei4, "aBcDeFg", "AbCdEfG", CASEI);

    matches!(matchslash1, "abc/def", "abc/def", SLASHLIT);
    nmatches!(matchslash2, "abc?def", "abc/def", SLASHLIT);
    nmatches!(matchslash2_win, "abc?def", "abc\\def", SLASHLIT);
    nmatches!(matchslash3, "abc*def", "abc/def", SLASHLIT);
    matches!(matchslash4, "abc[/]def", "abc/def", SLASHLIT); // differs

    nmatches!(matchnot1, "a*b*c", "abcd");
    nmatches!(matchnot2, "abc*abc*abc", "abcabcabcabcabcabcabca");
    nmatches!(matchnot3, "some/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot4, "some/**/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot5, "/**/test", "test");
    nmatches!(matchnot6, "/**/test", "/one/notthis");
    nmatches!(matchnot7, "/**/test", "/notthis");
    nmatches!(matchnot8, "**/.*", "ab.c");
    nmatches!(matchnot9, "**/.*", "abc/ab.c");
    nmatches!(matchnot10, ".*/**", "a.bc");
    nmatches!(matchnot11, ".*/**", "abc/a.bc");
    nmatches!(matchnot12, "a[0-9]b", "a_b");
    nmatches!(matchnot13, "a[!0-9]b", "a0b");
    nmatches!(matchnot14, "a[!0-9]b", "a9b");
    nmatches!(matchnot15, "[!-]", "-");
    nmatches!(matchnot16, "*hello.txt", "hello.txt-and-then-some");
    nmatches!(matchnot17, "*hello.txt", "goodbye.txt");
    nmatches!(matchnot18, "*some/path/to/hello.txt",
              "some/path/to/hello.txt-and-then-some");
    nmatches!(matchnot19, "*some/path/to/hello.txt",
              "some/other/path/to/hello.txt");

    #[test]
    fn set_works() {
        let mut builder = SetBuilder::new();
        builder.add("src/**/*.rs").unwrap();
        builder.add("*.c").unwrap();
        builder.add("src/lib.rs").unwrap();
        let set = builder.build().unwrap();

        fn is_match(set: &Set, s: &str) -> bool {
            let mut matches = vec![];
            set.matches_into(s, &mut matches);
            !matches.is_empty()
        }

        assert!(is_match(&set, "foo.c"));
        assert!(is_match(&set, "src/foo.c"));
        assert!(!is_match(&set, "foo.rs"));
        assert!(!is_match(&set, "tests/foo.rs"));
        assert!(is_match(&set, "src/foo.rs"));
        assert!(is_match(&set, "src/grep/src/main.rs"));

        let matches = set.matches("src/lib.rs");
        assert_eq!(2, matches.len());
        assert_eq!(0, matches[0]);
        assert_eq!(2, matches[1]);
    }
}
