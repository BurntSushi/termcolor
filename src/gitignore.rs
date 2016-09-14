/*!
The gitignore module provides a way of reading a gitignore file and applying
it to a particular file name to determine whether it should be ignore or not.
The motivation for this submodule is performance and portability:

1. There is a gitignore crate on crates.io, but it uses the standard `glob`
   crate and checks patterns one-by-one. This is a reasonable implementation,
   but not suitable for the performance we need here.
2. We could shell out to a `git` sub-command like ls-files or status, but it
   seems better to not rely on the existence of external programs for a search
   tool. Besides, we need to implement this logic anyway to support things like
   an .rgignore file.

The key implementation detail here is that a single gitignore file is compiled
into a single RegexSet, which can be used to report which globs match a
particular file name. We can then do a quick post-processing step to implement
additional rules such as whitelists (prefix of `!`) or directory-only globs
(suffix of `/`).
*/

// TODO(burntsushi): Implement something similar, but for Mercurial. We can't
// use this exact implementation because hgignore files are different.

use std::error::Error as StdError;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use regex;

use glob;

/// Represents an error that can occur when parsing a gitignore file.
#[derive(Debug)]
pub enum Error {
    Glob(glob::Error),
    Regex(regex::Error),
    Io(io::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Glob(ref err) => err.description(),
            Error::Regex(ref err) => err.description(),
            Error::Io(ref err) => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Glob(ref err) => err.fmt(f),
            Error::Regex(ref err) => err.fmt(f),
            Error::Io(ref err) => err.fmt(f),
        }
    }
}

impl From<glob::Error> for Error {
    fn from(err: glob::Error) -> Error {
        Error::Glob(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::Regex(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// Gitignore is a matcher for the glob patterns in a single gitignore file.
#[derive(Clone, Debug)]
pub struct Gitignore {
    set: glob::Set,
    root: PathBuf,
    patterns: Vec<Pattern>,
    num_ignores: u64,
    num_whitelist: u64,
}

impl Gitignore {
    /// Create a new gitignore glob matcher from the given root directory and
    /// string containing the contents of a gitignore file.
    #[allow(dead_code)]
    fn from_str<P: AsRef<Path>>(
        root: P,
        gitignore: &str,
    ) -> Result<Gitignore, Error> {
        let mut builder = GitignoreBuilder::new(root);
        try!(builder.add_str(gitignore));
        builder.build()
    }

    /// Returns true if and only if the given file path should be ignored
    /// according to the globs in this gitignore. `is_dir` should be true if
    /// the path refers to a directory and false otherwise.
    ///
    /// Before matching path, its prefix (as determined by a common suffix
    /// of the directory containing this gitignore) is stripped. If there is
    /// no common suffix/prefix overlap, then path is assumed to reside in the
    /// same directory as this gitignore file.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match {
        let mut path = path.as_ref();
        if let Ok(p) = path.strip_prefix(&self.root) {
            path = p;
        }
        self.matched_utf8(&*path.to_string_lossy(), is_dir)
    }

    /// Like matched, but takes a path that has already been stripped and
    /// converted to UTF-8.
    pub fn matched_utf8(&self, path: &str, is_dir: bool) -> Match {
        // A single regex with a bunch of alternations of glob patterns is
        // unfortunately typically faster than a regex, so we use it as a
        // first pass filter. We still need to run the RegexSet to get the most
        // recently defined glob that matched.
        if !self.set.is_match(path) {
            return Match::None;
        }
        // The regex set can't actually pick the right glob that matched all
        // on its own. In particular, some globs require that only directories
        // can match. Thus, only accept a match from the regex set if the given
        // path satisfies the corresponding glob's directory criteria.
        for i in self.set.matches(path).iter().rev() {
            let pat = &self.patterns[i];
            if !pat.only_dir || is_dir {
                return if pat.whitelist {
                    Match::Whitelist(pat)
                } else {
                    Match::Ignored(pat)
                };
            }
        }
        Match::None
    }

    /// Returns the total number of ignore patterns.
    pub fn num_ignores(&self) -> u64 {
        self.num_ignores
    }
}

/// The result of a glob match.
///
/// The lifetime `'a` refers to the lifetime of the pattern that resulted in
/// a match (whether ignored or whitelisted).
#[derive(Clone, Debug)]
pub enum Match<'a> {
    /// The path didn't match any glob in the gitignore file.
    None,
    /// The last glob matched indicates the path should be ignored.
    Ignored(&'a Pattern),
    /// The last glob matched indicates the path should be whitelisted.
    Whitelist(&'a Pattern),
}

impl<'a> Match<'a> {
    /// Returns true if the match result implies the path should be ignored.
    #[allow(dead_code)]
    pub fn is_ignored(&self) -> bool {
        match *self {
            Match::Ignored(_) => true,
            Match::None | Match::Whitelist(_) => false,
        }
    }

    /// Returns true if the match result didn't match any globs.
    pub fn is_none(&self) -> bool {
        match *self {
            Match::None => true,
            Match::Ignored(_) | Match::Whitelist(_) => false,
        }
    }

    /// Inverts the match so that Ignored becomes Whitelisted and Whitelisted
    /// becomes Ignored. A non-match remains the same.
    pub fn invert(self) -> Match<'a> {
        match self {
            Match::None => Match::None,
            Match::Ignored(pat) => Match::Whitelist(pat),
            Match::Whitelist(pat) => Match::Ignored(pat),
        }
    }
}

/// GitignoreBuilder constructs a matcher for a single set of globs from a
/// .gitignore file.
pub struct GitignoreBuilder {
    builder: glob::SetBuilder,
    root: PathBuf,
    patterns: Vec<Pattern>,
}

/// Pattern represents a single pattern in a gitignore file. It doesn't
/// know how to do glob matching directly, but it does store additional
/// options on a pattern, such as whether it's whitelisted.
#[derive(Clone, Debug)]
pub struct Pattern {
    /// The file path that this pattern was extracted from (may be empty).
    pub from: PathBuf,
    /// The original glob pattern string.
    pub original: String,
    /// The actual glob pattern string used to convert to a regex.
    pub pat: String,
    /// Whether this is a whitelisted pattern or not.
    pub whitelist: bool,
    /// Whether this pattern should only match directories or not.
    pub only_dir: bool,
}

impl GitignoreBuilder {
    /// Create a new builder for a gitignore file.
    ///
    /// The path given should be the path at which the globs for this gitignore
    /// file should be matched.
    pub fn new<P: AsRef<Path>>(root: P) -> GitignoreBuilder {
        GitignoreBuilder {
            builder: glob::SetBuilder::new(),
            root: root.as_ref().to_path_buf(),
            patterns: vec![],
        }
    }

    /// Builds a new matcher from the glob patterns added so far.
    ///
    /// Once a matcher is built, no new glob patterns can be added to it.
    pub fn build(self) -> Result<Gitignore, Error> {
        let nignores = self.patterns.iter().filter(|p| !p.whitelist).count();
        let nwhitelist = self.patterns.iter().filter(|p| p.whitelist).count();
        Ok(Gitignore {
            set: try!(self.builder.build()),
            root: self.root,
            patterns: self.patterns,
            num_ignores: nignores as u64,
            num_whitelist: nwhitelist as u64,
        })
    }

    /// Add each pattern line from the file path given.
    pub fn add_path<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let rdr = io::BufReader::new(try!(File::open(&path)));
        for line in rdr.lines() {
            try!(self.add(&path, &try!(line)));
        }
        Ok(())
    }

    /// Add each pattern line from the string given.
    pub fn add_str(&mut self, gitignore: &str) -> Result<(), Error> {
        for line in gitignore.lines() {
            try!(self.add("", line));
        }
        Ok(())
    }

    /// Add a line from a gitignore file to this builder.
    ///
    /// If the line could not be parsed as a glob, then an error is returned.
    pub fn add<P: AsRef<Path>>(
        &mut self,
        from: P,
        mut line: &str,
    ) -> Result<(), Error> {
        if line.is_empty() {
            return Ok(());
        }
        let mut pat = Pattern {
            from: from.as_ref().to_path_buf(),
            original: line.to_string(),
            pat: String::new(),
            whitelist: false,
            only_dir: false,
        };
        let mut opts = glob::MatchOptions::default();
        let has_slash = line.chars().any(|c| c == '/');
        // If the line starts with an escaped '!', then remove the escape.
        // Otherwise, if it starts with an unescaped '!', then this is a
        // whitelist pattern.
        match line.chars().nth(0) {
            Some('#') => return Ok(()),
            Some('\\') => {
                match line.chars().nth(1) {
                    Some('!') | Some('#') => {
                        line = &line[1..];
                    }
                    _ => {}
                }
            }
            Some('!') => {
                pat.whitelist = true;
                line = &line[1..];
            }
            Some('/') => {
                // `man gitignore` says that if a glob starts with a slash,
                // then the glob can only match the beginning of a path
                // (relative to the location of gitignore). We achieve this by
                // simply banning wildcards from matching /.
                opts.require_literal_separator = true;
                line = &line[1..];
            }
            _ => {}
        }
        // If it ends with a slash, then this should only match directories,
        // but the slash should otherwise not be used while globbing.
        if let Some((i, c)) = line.char_indices().rev().nth(0) {
            if c == '/' {
                pat.only_dir = true;
                line = &line[..i];
            }
        }
        // If there is a literal slash, then we note that so that globbing
        // doesn't let wildcards match slashes. Otherwise, we need to let
        // the pattern match anywhere, so we add a `**/` prefix to achieve
        // that behavior.
        pat.pat = line.to_string();
        if has_slash {
            opts.require_literal_separator = true;
        } else {
            pat.pat = format!("**/{}", pat.pat);
        }
        try!(self.builder.add_with(&pat.pat, &opts));
        self.patterns.push(pat);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Gitignore;

    macro_rules! ignored {
        ($name:ident, $root:expr, $gi:expr, $path:expr) => {
            ignored!($name, $root, $gi, $path, false);
        };
        ($name:ident, $root:expr, $gi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let gi = Gitignore::from_str($root, $gi).unwrap();
                assert!(gi.matched($path, $is_dir).is_ignored());
            }
        };
    }

    macro_rules! not_ignored {
        ($name:ident, $root:expr, $gi:expr, $path:expr) => {
            not_ignored!($name, $root, $gi, $path, false);
        };
        ($name:ident, $root:expr, $gi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let gi = Gitignore::from_str($root, $gi).unwrap();
                assert!(!gi.matched($path, $is_dir).is_ignored());
            }
        };
    }

    const ROOT: &'static str = "/home/foobar/rust/rg";

    ignored!(ig1, ROOT, "months", "months");
    ignored!(ig2, ROOT, "*.lock", "Cargo.lock");
    ignored!(ig3, ROOT, "*.rs", "src/main.rs");
    ignored!(ig4, ROOT, "src/*.rs", "src/main.rs");
    ignored!(ig5, ROOT, "/*.c", "cat-file.c");
    ignored!(ig6, ROOT, "/src/*.rs", "src/main.rs");
    ignored!(ig7, ROOT, "!src/main.rs\n*.rs", "src/main.rs");
    ignored!(ig8, ROOT, "foo/", "foo", true);
    ignored!(ig9, ROOT, "**/foo", "foo");
    ignored!(ig10, ROOT, "**/foo", "src/foo");
    ignored!(ig11, ROOT, "**/foo/**", "src/foo/bar");
    ignored!(ig12, ROOT, "**/foo/**", "wat/src/foo/bar/baz");
    ignored!(ig13, ROOT, "**/foo/bar", "foo/bar");
    ignored!(ig14, ROOT, "**/foo/bar", "src/foo/bar");
    ignored!(ig15, ROOT, "abc/**", "abc/x");
    ignored!(ig16, ROOT, "abc/**", "abc/x/y");
    ignored!(ig17, ROOT, "abc/**", "abc/x/y/z");
    ignored!(ig18, ROOT, "a/**/b", "a/b");
    ignored!(ig19, ROOT, "a/**/b", "a/x/b");
    ignored!(ig20, ROOT, "a/**/b", "a/x/y/b");
    ignored!(ig21, ROOT, r"\!xy", "!xy");
    ignored!(ig22, ROOT, r"\#foo", "#foo");
    ignored!(ig23, ROOT, "foo", "./foo");
    ignored!(ig24, ROOT, "target", "grep/target");
    ignored!(ig25, ROOT, "Cargo.lock", "./tabwriter-bin/Cargo.lock");

    not_ignored!(ignot1, ROOT, "amonths", "months");
    not_ignored!(ignot2, ROOT, "monthsa", "months");
    not_ignored!(ignot3, ROOT, "src/*.rs", "src/grep/src/main.rs");
    not_ignored!(ignot4, ROOT, "/*.c", "mozilla-sha1/sha1.c");
    not_ignored!(ignot5, ROOT, "/src/*.rs", "src/grep/src/main.rs");
    not_ignored!(ignot6, ROOT, "*.rs\n!src/main.rs", "src/main.rs");
    not_ignored!(ignot7, ROOT, "foo/", "foo", false);
    not_ignored!(ignot8, ROOT, "**/foo/**", "wat/src/afoo/bar/baz");
    not_ignored!(ignot9, ROOT, "**/foo/**", "wat/src/fooa/bar/baz");
    not_ignored!(ignot10, ROOT, "**/foo/bar", "foo/src/bar");
    not_ignored!(ignot11, ROOT, "#foo", "#foo");
    not_ignored!(ignot12, ROOT, "\n\n\n", "foo");
}
