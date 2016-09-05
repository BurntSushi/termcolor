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

use std::error::Error as StdError;
use std::fmt;
use std::iter;
use std::path;
use std::str;

use regex;
use regex::bytes::{Regex, RegexSet, SetMatches};

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

/// Set represents a group of globs that can be matched together in a single
/// pass.
#[derive(Clone, Debug)]
pub struct Set {
    re: Regex,
    set: RegexSet,
}

impl Set {
    /// Returns true if and only if the given path matches at least one glob
    /// in this set.
    pub fn is_match<T: AsRef<[u8]>>(&self, path: T) -> bool {
        self.re.is_match(path.as_ref())
    }

    /// Returns every glob pattern (by sequence number) that matches the given
    /// path.
    pub fn matches<T: AsRef<[u8]>>(&self, path: T) -> SetMatches {
        // TODO(burntsushi): If we split this out into a separate crate, don't
        // expose the regex::SetMatches type in the public API.
        self.set.matches(path.as_ref())
    }

    /// Returns the number of glob patterns in this set.
    pub fn len(&self) -> usize {
        self.set.len()
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
        let it = self.pats.iter().map(|&(ref p, ref o)| p.to_regex_with(o));
        let set = try!(RegexSet::new(it));

        let mut joined = String::new();
        for &(ref p, ref o) in &self.pats {
            let part = format!("(?:{})", p.to_regex_with(o));
            if !joined.is_empty() {
                joined.push('|');
            }
            joined.push_str(&part);
        }
        let re = try!(Regex::new(&joined));
        Ok(Set { re: re, set: set })
    }

    /// Add a new pattern to this set.
    ///
    /// If the pattern could not be parsed as a glob, then an error is
    /// returned.
    pub fn add(&mut self, pat: &str) -> Result<(), Error> {
        self.add_with(pat, &MatchOptions::default())
    }

    /// Like add, but sets the match options for this particular pattern.
    pub fn add_with(
        &mut self,
        pat: &str,
        opts: &MatchOptions,
    ) -> Result<(), Error> {
        let pat = try!(Pattern::new(pat));
        self.pats.push((pat, opts.clone()));
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

    /// Convert this pattern to a string that is guaranteed to be a valid
    /// regular expression and will represent the matching semantics of this
    /// glob pattern. This uses a default set of options.
    pub fn to_regex(&self) -> String {
        self.to_regex_with(&MatchOptions::default())
    }

    /// Convert this pattern to a string that is guaranteed to be a valid
    /// regular expression and will represent the matching semantics of this
    /// glob pattern and the options given.
    pub fn to_regex_with(&self, options: &MatchOptions) -> String {
        let sep = path::MAIN_SEPARATOR.to_string();
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
                        re.push_str(&format!("[^{}]", regex::quote(&sep)));
                    } else {
                        re.push_str(".");
                    }
                }
                Token::ZeroOrMore => {
                    if options.require_literal_separator {
                        re.push_str(&format!("[^{}]*", regex::quote(&sep)));
                    } else {
                        re.push_str(".*");
                    }
                }
                Token::RecursivePrefix => {
                    re.push_str(&format!("(?:{sep}?|.*{sep})", sep=sep));
                }
                Token::RecursiveSuffix => {
                    re.push_str(&format!("(?:{sep}?|{sep}.*)", sep=sep));
                }
                Token::RecursiveZeroOrMore => {
                    re.push_str(&format!("(?:{sep}|{sep}.*{sep})", sep=sep));
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
        let last = self.p.tokens.pop().unwrap();
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

#[cfg(test)]
mod tests {
    use regex::bytes::Regex;

    use super::{Error, Pattern, MatchOptions, SetBuilder, Token};
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
                let re = Regex::new(&pat.to_regex_with(&$options)).unwrap();
                assert!(re.is_match($path.as_bytes()));
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
                let re = Regex::new(&pat.to_regex_with(&$options)).unwrap();
                assert!(!re.is_match($path.as_bytes()));
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
    const SEP: char = ::std::path::MAIN_SEPARATOR;

    toregex!(re_casei, "a", "(?i)^a$", &CASEI);

    toregex!(re_slash1, "?", format!("^[^{}]$", SEP), SLASHLIT);
    toregex!(re_slash2, "*", format!("^[^{}]*$", SEP), SLASHLIT);

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

        assert!(set.is_match("foo.c"));
        assert!(set.is_match("src/foo.c"));
        assert!(!set.is_match("foo.rs"));
        assert!(!set.is_match("tests/foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(set.is_match("src/grep/src/main.rs"));

        assert_eq!(2, set.matches("src/lib.rs").iter().count());
        assert!(set.matches("src/lib.rs").matched(0));
        assert!(!set.matches("src/lib.rs").matched(1));
        assert!(set.matches("src/lib.rs").matched(2));
    }
}
