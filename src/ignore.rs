/*!
The ignore module is responsible for managing the state required to determine
whether a *single* file path should be searched or not.

In general, there are two ways to ignore a particular file:

1. Specify an ignore rule in some "global" configuration, such as a
   $HOME/.ignore or on the command line.
2. A specific ignore file (like .gitignore) found during directory traversal.

The `IgnoreDir` type handles ignore patterns for any one particular directory
(including "global" ignore patterns), while the `Ignore` type handles a stack
of `IgnoreDir`s for use during directory traversal.
*/

use std::error::Error as StdError;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use gitignore::{self, Gitignore, GitignoreBuilder, Match, Pattern};
use pathutil::{file_name, is_hidden, strip_prefix};
use types::Types;

const IGNORE_NAMES: &'static [&'static str] = &[
    ".gitignore",
    ".ignore",
    ".rgignore",
];

/// Represents an error that can occur when parsing a gitignore file.
#[derive(Debug)]
pub enum Error {
    Gitignore(gitignore::Error),
    Io {
        path: PathBuf,
        err: io::Error,
    },
}

impl Error {
    fn from_io<P: AsRef<Path>>(path: P, err: io::Error) -> Error {
        Error::Io { path: path.as_ref().to_path_buf(), err: err }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Gitignore(ref err) => err.description(),
            Error::Io { ref err, .. } => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Gitignore(ref err) => err.fmt(f),
            Error::Io { ref path, ref err } => {
                write!(f, "{}: {}", path.display(), err)
            }
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
    stack: Vec<IgnoreDir>,
    /// A stack of parent directories above the root of the current search.
    parent_stack: Vec<IgnoreDir>,
    /// A set of override globs that are always checked first. A match (whether
    /// it's whitelist or blacklist) trumps anything in stack.
    overrides: Overrides,
    /// A file type matcher.
    types: Types,
    /// Whether to ignore hidden files or not.
    ignore_hidden: bool,
    /// When true, don't look at .gitignore or .ignore files for ignore
    /// rules.
    no_ignore: bool,
    /// When true, don't look at .gitignore files for ignore rules.
    no_ignore_vcs: bool,
}

impl Ignore {
    /// Create an empty set of ignore patterns.
    pub fn new() -> Ignore {
        Ignore {
            stack: vec![],
            parent_stack: vec![],
            overrides: Overrides::new(None),
            types: Types::empty(),
            ignore_hidden: true,
            no_ignore: false,
            no_ignore_vcs: true,
        }
    }

    /// Set whether hidden files/folders should be ignored (defaults to true).
    pub fn ignore_hidden(&mut self, yes: bool) -> &mut Ignore {
        self.ignore_hidden = yes;
        self
    }

    /// When set, ignore files are ignored.
    pub fn no_ignore(&mut self, yes: bool) -> &mut Ignore {
        self.no_ignore = yes;
        self
    }

    /// When set, VCS ignore files are ignored.
    pub fn no_ignore_vcs(&mut self, yes: bool) -> &mut Ignore {
        self.no_ignore_vcs = yes;
        self
    }

    /// Add a set of globs that overrides all other match logic.
    pub fn add_override(&mut self, gi: Gitignore) -> &mut Ignore {
        self.overrides = Overrides::new(Some(gi));
        self
    }

    /// Add a file type matcher. The file type matcher has the lowest
    /// precedence.
    pub fn add_types(&mut self, types: Types) -> &mut Ignore {
        self.types = types;
        self
    }

    /// Push parent directories of `path` on to the stack.
    pub fn push_parents<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Error> {
        let path = try!(path.as_ref().canonicalize().map_err(|err| {
            Error::from_io(path.as_ref(), err)
        }));
        let mut path = &*path;
        let mut saw_git = path.join(".git").is_dir();
        let mut ignore_names = IGNORE_NAMES.to_vec();
        if self.no_ignore_vcs {
            ignore_names.retain(|&name| name != ".gitignore");
        }
        let mut ignore_dir_results = vec![];
        while let Some(parent) = path.parent() {
            if self.no_ignore {
                ignore_dir_results.push(Ok(IgnoreDir::empty(parent)));
            } else {
                if saw_git {
                    ignore_names.retain(|&name| name != ".gitignore");
                } else {
                    saw_git = parent.join(".git").is_dir();
                }
                let ignore_dir_result =
                    IgnoreDir::with_ignore_names(parent, ignore_names.iter());
                ignore_dir_results.push(ignore_dir_result);
            }
            path = parent;
        }

        for ignore_dir_result in ignore_dir_results.into_iter().rev() {
            self.parent_stack.push(try!(ignore_dir_result));
        }
        Ok(())
    }

    /// Add a directory to the stack.
    ///
    /// Note that even if this returns an error, the directory is added to the
    /// stack (and therefore should be popped).
    pub fn push<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        if self.no_ignore {
            self.stack.push(IgnoreDir::empty(path));
            Ok(())
        } else if self.no_ignore_vcs {
            self.push_ignore_dir(IgnoreDir::without_vcs(path))
        } else {
            self.push_ignore_dir(IgnoreDir::new(path))
        }
    }

    /// Pushes the result of building a directory matcher on to the stack.
    ///
    /// If the result given contains an error, then it is returned.
    pub fn push_ignore_dir(
        &mut self,
        result: Result<IgnoreDir, Error>,
    ) -> Result<(), Error> {
        match result {
            Ok(id) => {
                self.stack.push(id);
                Ok(())
            }
            Err(err) => {
                // Don't leave the stack in an inconsistent state.
                self.stack.push(IgnoreDir::empty("error"));
                Err(err)
            }
        }
    }

    /// Pop a directory from the stack.
    ///
    /// This panics if the stack is empty.
    pub fn pop(&mut self) {
        self.stack.pop().expect("non-empty stack");
    }

    /// Returns true if and only if the given file path should be ignored.
    pub fn ignored<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> bool {
        let mut path = path.as_ref();
        if let Some(p) = strip_prefix("./", path) {
            path = p;
        }
        let mat = self.overrides.matched(path, is_dir);
        if let Some(is_ignored) = self.ignore_match(path, mat) {
            return is_ignored;
        }
        let mut whitelisted = false;
        if !self.no_ignore {
            for id in self.stack.iter().rev() {
                let mat = id.matched(path, is_dir);
                if let Some(is_ignored) = self.ignore_match(path, mat) {
                    if is_ignored {
                        return true;
                    }
                    // If this path is whitelisted by an ignore, then
                    // fallthrough and let the file type matcher have a say.
                    whitelisted = true;
                    break;
                }
            }
            // If the file has been whitelisted, then we have to stop checking
            // parent directories. The only thing that can override a whitelist
            // at this point is a type filter.
            if !whitelisted {
                let mut path = path.to_path_buf();
                for id in self.parent_stack.iter().rev() {
                    if let Some(ref dirname) = id.name {
                        path = Path::new(dirname).join(path);
                    }
                    let mat = id.matched(&*path, is_dir);
                    if let Some(is_ignored) = self.ignore_match(&*path, mat) {
                        if is_ignored {
                            return true;
                        }
                        // If this path is whitelisted by an ignore, then
                        // fallthrough and let the file type matcher have a
                        // say.
                        whitelisted = true;
                        break;
                    }
                }
            }
        }
        let mat = self.types.matched(path, is_dir);
        if let Some(is_ignored) = self.ignore_match(path, mat) {
            if is_ignored {
                return true;
            }
            whitelisted = true;
        }
        if !whitelisted && self.ignore_hidden && is_hidden(&path) {
            debug!("{} ignored because it is hidden", path.display());
            return true;
        }
        false
    }

    /// Returns true if the given match says the given pattern should be
    /// ignored or false if the given pattern should be explicitly whitelisted.
    /// Returns None otherwise.
    pub fn ignore_match<P: AsRef<Path>>(
        &self,
        path: P,
        mat: Match,
    ) -> Option<bool> {
        let path = path.as_ref();
        match mat {
            Match::Whitelist(ref pat) => {
                debug!("{} whitelisted by {:?}", path.display(), pat);
                Some(false)
            }
            Match::Ignored(ref pat) => {
                debug!("{} ignored by {:?}", path.display(), pat);
                Some(true)
            }
            Match::None => None,
        }
    }
}

/// IgnoreDir represents a set of ignore patterns retrieved from a single
/// directory.
#[derive(Debug)]
pub struct IgnoreDir {
    /// The path to this directory as given.
    path: PathBuf,
    /// The directory name, if one exists.
    name: Option<OsString>,
    /// A single accumulation of glob patterns for this directory, matched
    /// using gitignore semantics.
    ///
    /// This will include patterns from rgignore as well. The patterns are
    /// ordered so that precedence applies automatically (e.g., rgignore
    /// patterns procede gitignore patterns).
    gi: Option<Gitignore>,
    // TODO(burntsushi): Matching other types of glob patterns that don't
    // conform to gitignore will probably require refactoring this approach.
}

impl IgnoreDir {
    /// Create a new matcher for the given directory.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<IgnoreDir, Error> {
        IgnoreDir::with_ignore_names(path, IGNORE_NAMES.iter())
    }

    /// Create a new matcher for the given directory.
    ///
    /// Don't respect VCS ignore files.
    pub fn without_vcs<P: AsRef<Path>>(path: P) -> Result<IgnoreDir, Error> {
        let names = IGNORE_NAMES.iter().filter(|name| **name != ".gitignore");
        IgnoreDir::with_ignore_names(path, names)
    }

    /// Create a new IgnoreDir that never matches anything with the given path.
    pub fn empty<P: AsRef<Path>>(path: P) -> IgnoreDir {
        IgnoreDir {
            path: path.as_ref().to_path_buf(),
            name: file_name(path.as_ref()).map(|s| s.to_os_string()),
            gi: None,
        }
    }

    /// Create a new matcher for the given directory using only the ignore
    /// patterns found in the file names given.
    ///
    /// If no ignore glob patterns could be found in the directory then `None`
    /// is returned.
    ///
    /// Note that the order of the names given is meaningful. Names appearing
    /// later in the list have precedence over names appearing earlier in the
    /// list.
    pub fn with_ignore_names<P: AsRef<Path>, S, I>(
        path: P,
        names: I,
    ) -> Result<IgnoreDir, Error>
    where P: AsRef<Path>, S: AsRef<str>, I: Iterator<Item=S> {
        let mut id = IgnoreDir::empty(path);
        let mut ok = false;
        let mut builder = GitignoreBuilder::new(&id.path);
        // The ordering here is important. Later globs have higher precedence.
        for name in names {
            ok = builder.add_path(id.path.join(name.as_ref())).is_ok() || ok;
        }
        if !ok {
            return Ok(id);
        }
        id.gi = Some(try!(builder.build()));
        Ok(id)
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

/// Manages a set of overrides provided explicitly by the end user.
struct Overrides {
    gi: Option<Gitignore>,
    unmatched_pat: Pattern,
}

impl Overrides {
    /// Creates a new set of overrides from the gitignore matcher provided.
    /// If no matcher is provided, then the resulting overrides have no effect.
    fn new(gi: Option<Gitignore>) -> Overrides {
        Overrides {
            gi: gi,
            unmatched_pat: Pattern {
                from: Path::new("<argv>").to_path_buf(),
                original: "<none>".to_string(),
                pat: "<none>".to_string(),
                whitelist: false,
                only_dir: false,
            },
        }
    }

    /// Returns a match for the given path against this set of overrides.
    ///
    /// If there are no overrides, then this always returns Match::None.
    ///
    /// If there is at least one positive override, then this never returns
    /// Match::None (and interpreting non-matches as ignored) unless is_dir
    /// is true.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match {
        let path = path.as_ref();
        self.gi.as_ref()
            .map(|gi| {
                let mat = gi.matched_stripped(path, is_dir).invert();
                if mat.is_none() && !is_dir {
                    if gi.num_ignores() > 0 {
                        return Match::Ignored(&self.unmatched_pat);
                    }
                }
                mat
            })
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
                    name: Path::new($root).file_name().map(|s| {
                        s.to_os_string()
                    }),
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
                    name: Path::new($root).file_name().map(|s| {
                        s.to_os_string()
                    }),
                    gi: Some(gi),
                };
                assert!(!id.matched($path, false).is_ignored());
            }
        };
    }

    const ROOT: &'static str = "/home/foobar/rust/rg";

    ignored_dir!(id1, ROOT, "src/main.rs", "", "src/main.rs");
    ignored_dir!(id2, ROOT, "", "src/main.rs", "src/main.rs");
    ignored_dir!(id3, ROOT, "!src/main.rs", "*.rs", "src/main.rs");

    not_ignored_dir!(idnot1, ROOT, "*.rs", "!src/main.rs", "src/main.rs");
}
