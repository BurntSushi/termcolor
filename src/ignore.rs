/*!
The ignore module is responsible for managing the state required to determine
whether a *single* file path should be searched or not.

In general, there are two ways to ignore a particular file:

1. Specify an ignore rule in some "global" configuration, such as a
   $HOME/.xrepignore or on the command line.
2. A specific ignore file (like .gitignore) found during directory traversal.

The `IgnoreDir` type handles ignore patterns for any one particular directory
(including "global" ignore patterns), while the `Ignore` type handles a stack
of `IgnoreDir`s for use during directory traversal.
*/

use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};

use gitignore::{self, Gitignore, GitignoreBuilder, Match};

/// Represents an error that can occur when parsing a gitignore file.
#[derive(Debug)]
pub enum Error {
    Gitignore(gitignore::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Gitignore(ref err) => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Gitignore(ref err) => err.fmt(f),
        }
    }
}

impl From<gitignore::Error> for Error {
    fn from(err: gitignore::Error) -> Error {
        Error::Gitignore(err)
    }
}

/// Ignore represents a collection of ignore patterns organized by directory.
/// In particular, a stack is maintained, where the top of the stack
/// corresponds to the current directory being searched and the bottom of the
/// stack represents the root of a search. Ignore patterns at the top of the
/// stack take precedence over ignore patterns at the bottom of the stack.
pub struct Ignore {
    /// A stack of ignore patterns at each directory level of traversal.
    /// A directory that contributes no ignore patterns is `None`.
    stack: Vec<Option<IgnoreDir>>,
    // TODO(burntsushi): Add other patterns from the command line here.
}

impl Ignore {
    /// Create an empty set of ignore patterns.
    pub fn new() -> Ignore {
        Ignore { stack: vec![] }
    }

    /// Add a directory to the stack.
    pub fn push<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        self.stack.push(try!(IgnoreDir::new(path)));
        Ok(())
    }

    /// Pop a directory from the stack.
    ///
    /// This panics if the stack is empty.
    pub fn pop(&mut self) {
        self.stack.pop().expect("non-empty stack");
    }

    /// Returns true if and only if the given file path should be ignored.
    pub fn ignored<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> bool {
        let path = path.as_ref();
        for id in self.stack.iter().rev().filter_map(|id| id.as_ref()) {
            match id.matched(path, is_dir) {
                Match::Whitelist => return false,
                Match::Ignored => return true,
                Match::None => {}
            }
        }
        false
    }
}

/// IgnoreDir represents a set of ignore patterns retrieved from a single
/// directory.
pub struct IgnoreDir {
    /// The path to this directory as given.
    path: PathBuf,
    /// A single accumulation of glob patterns for this directory, matched
    /// using gitignore semantics.
    ///
    /// This will include patterns from xrepignore as well. The patterns are
    /// ordered so that precedence applies automatically (e.g., xrepignore
    /// patterns procede gitignore patterns).
    gi: Option<Gitignore>,
    // TODO(burntsushi): Matching other types of glob patterns that don't
    // conform to gitignore will probably require refactoring this approach.
}

impl IgnoreDir {
    /// Create a new matcher for the given directory.
    ///
    /// If no ignore glob patterns could be found in the directory then `None`
    /// is returned.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Option<IgnoreDir>, Error> {
        let mut id = IgnoreDir {
            path: path.as_ref().to_path_buf(),
            gi: None,
        };
        let mut ok = false;
        let mut builder = GitignoreBuilder::new(&id.path);
        // The ordering here is important. Later globs have higher precedence.
        ok = builder.add_path(id.path.join(".gitignore")).is_ok() || ok;
        ok = builder.add_path(id.path.join(".agignore")).is_ok() || ok;
        ok = builder.add_path(id.path.join(".xrepignore")).is_ok() || ok;
        if !ok {
            Ok(None)
        } else {
            id.gi = Some(try!(builder.build()));
            Ok(Some(id))
        }
    }

    /// Returns true if and only if the given file path should be ignored
    /// according to the globs in this directory. `is_dir` should be true if
    /// the path refers to a directory and false otherwise.
    ///
    /// Before matching path, its prefix (as determined by a common suffix
    /// of this directory) is stripped. If there is
    /// no common suffix/prefix overlap, then path is assumed to reside
    /// directly in this directory.
    ///
    /// If the given path has a `./` prefix then it is stripped before
    /// matching.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match {
        self.gi.as_ref()
            .map(|gi| gi.matched(path, is_dir))
            .unwrap_or(Match::None)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use gitignore::GitignoreBuilder;
    use super::IgnoreDir;

    macro_rules! ignored_dir {
        ($name:ident, $root:expr, $gi:expr, $xi:expr, $path:expr) => {
            #[test]
            fn $name() {
                let mut builder = GitignoreBuilder::new(&$root);
                builder.add_str($gi).unwrap();
                builder.add_str($xi).unwrap();
                let gi = builder.build().unwrap();
                let id = IgnoreDir {
                    path: Path::new($root).to_path_buf(),
                    gi: Some(gi),
                };
                assert!(id.matched($path, false).is_ignored());
            }
        };
    }

    macro_rules! not_ignored_dir {
        ($name:ident, $root:expr, $gi:expr, $xi:expr, $path:expr) => {
            #[test]
            fn $name() {
                let mut builder = GitignoreBuilder::new(&$root);
                builder.add_str($gi).unwrap();
                builder.add_str($xi).unwrap();
                let gi = builder.build().unwrap();
                let id = IgnoreDir {
                    path: Path::new($root).to_path_buf(),
                    gi: Some(gi),
                };
                assert!(!id.matched($path, false).is_ignored());
            }
        };
    }

    const ROOT: &'static str = "/home/foobar/rust/xrep";

    ignored_dir!(id1, ROOT, "src/main.rs", "", "src/main.rs");
    ignored_dir!(id2, ROOT, "", "src/main.rs", "src/main.rs");
    ignored_dir!(id3, ROOT, "!src/main.rs", "*.rs", "src/main.rs");

    not_ignored_dir!(idnot1, ROOT, "*.rs", "!src/main.rs", "src/main.rs");
}
