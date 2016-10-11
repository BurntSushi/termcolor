use std::ffi::OsStr;
use std::fs::{FileType, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::vec;

use walkdir::{self, WalkDir, WalkDirIterator};

use dir::{Ignore, IgnoreBuilder};
use gitignore::GitignoreBuilder;
use overrides::Override;
use types::Types;
use {Error, PartialErrorBuilder};

/// WalkBuilder builds a recursive directory iterator.
///
/// The builder supports a large number of configurable options. This includes
/// specific glob overrides, file type matching, toggling whether hidden
/// files are ignored or not, and of course, support for respecting gitignore
/// files.
///
/// By default, all ignore files found are respected. This includes `.ignore`,
/// `.gitignore`, `.git/info/exclude` and even your global gitignore
/// globs, usually found in `$XDG_CONFIG_HOME/git/ignore`.
///
/// Some standard recursive directory options are also supported, such as
/// limiting the recursive depth or whether to follow symbolic links (disabled
/// by default).
///
/// # Ignore rules
///
/// There are many rules that influence whether a particular file or directory
/// is skipped by this iterator. Those rules are documented here. Note that
/// the rules assume a default configuration.
///
/// * First, glob overrides are checked. If a path matches a glob override,
/// then matching stops. The path is then only skipped if the glob that matched
/// the path is an ignore glob. (An override glob is a whitelist glob unless it
/// starts with a `!`, in which case it is an ignore glob.)
/// * Second, ignore files are checked. Ignore files currently only come from
/// git ignore files (`.gitignore`, `.git/info/exclude` and the configured
/// global gitignore file), plain `.ignore` files, which have the same format
/// as gitignore files, or explicitly added ignore files. The precedence order
/// is: `.ignore`, `.gitignore`, `.git/info/exclude`, global gitignore and
/// finally explicitly added ignore files. Note that precedence between
/// different types of ignore files is not impacted by the directory hierarchy;
/// any `.ignore` file overrides all `.gitignore` files. Within each
/// precedence level, more nested ignore files have a higher precedence over
/// less nested ignore files.
/// * Third, if the previous step yields an ignore match, than all matching
/// is stopped and the path is skipped.. If it yields a whitelist match, then
/// process continues. A whitelist match can be overridden by a later matcher.
/// * Fourth, unless the path is a directory, the file type matcher is run on
/// the path. As above, if it's an ignore match, then all matching is stopped
/// and the path is skipped. If it's a whitelist match, then matching
/// continues.
/// * Fifth, if the path hasn't been whitelisted and it is hidden, then the
/// path is skipped.
/// * Sixth, if the path has made it this far then it is yielded in the
/// iterator.
pub struct WalkBuilder {
    paths: Vec<PathBuf>,
    ig_builder: IgnoreBuilder,
    parents: bool,
    max_depth: Option<usize>,
    follow_links: bool,
}

impl WalkBuilder {
    /// Create a new builder for a recursive directory iterator for the
    /// directory given.
    ///
    /// Note that if you want to traverse multiple different directories, it
    /// is better to call `add` on this builder than to create multiple
    /// `Walk` values.
    pub fn new<P: AsRef<Path>>(path: P) -> WalkBuilder {
        WalkBuilder {
            paths: vec![path.as_ref().to_path_buf()],
            ig_builder: IgnoreBuilder::new(),
            parents: true,
            max_depth: None,
            follow_links: false,
        }
    }

    /// Build a new `Walk` iterator.
    pub fn build(&self) -> Walk {
        let follow_links = self.follow_links;
        let max_depth = self.max_depth;
        let its = self.paths.iter().map(move |p| {
            if p == Path::new("-") {
                (p.to_path_buf(), None)
            } else {
                let mut wd = WalkDir::new(p);
                wd = wd.follow_links(follow_links || p.is_file());
                if let Some(max_depth) = max_depth {
                    wd = wd.max_depth(max_depth);
                }
                (p.to_path_buf(), Some(WalkEventIter::from(wd)))
            }
        }).collect::<Vec<_>>().into_iter();
        let ig_root = self.ig_builder.build();
        Walk {
            its: its,
            it: None,
            ig_root: ig_root.clone(),
            ig: ig_root.clone(),
            parents: self.parents,
        }
    }

    /// Add a file path to the iterator.
    ///
    /// Each additional file path added is traversed recursively. This should
    /// be preferred over building multiple `Walk` iterators since this
    /// enables reusing resources across iteration.
    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> &mut WalkBuilder {
        self.paths.push(path.as_ref().to_path_buf());
        self
    }

    /// The maximum depth to recurse.
    ///
    /// The default, `None`, imposes no depth restriction.
    pub fn max_depth(&mut self, depth: Option<usize>) -> &mut WalkBuilder {
        self.max_depth = depth;
        self
    }

    /// Whether to follow symbolic links or not.
    pub fn follow_links(&mut self, yes: bool) -> &mut WalkBuilder {
        self.follow_links = yes;
        self
    }

    /// Add an ignore file to the matcher.
    ///
    /// This has lower precedence than all other sources of ignore rules.
    ///
    /// If there was a problem adding the ignore file, then an error is
    /// returned. Note that the error may indicate *partial* failure. For
    /// example, if an ignore file contains an invalid glob, all other globs
    /// are still applied.
    pub fn add_ignore<P: AsRef<Path>>(&mut self, path: P) -> Option<Error> {
        let mut builder = GitignoreBuilder::new("");
        let mut errs = PartialErrorBuilder::default();
        errs.maybe_push_ignore_io(builder.add(path));
        match builder.build() {
            Ok(gi) => { self.ig_builder.add_ignore(gi); }
            Err(err) => { errs.push(err); }
        }
        errs.into_error_option()
    }

    /// Add an override matcher.
    ///
    /// By default, no override matcher is used.
    ///
    /// This overrides any previous setting.
    pub fn overrides(&mut self, overrides: Override) -> &mut WalkBuilder {
        self.ig_builder.overrides(overrides);
        self
    }

    /// Add a file type matcher.
    ///
    /// By default, no file type matcher is used.
    ///
    /// This overrides any previous setting.
    pub fn types(&mut self, types: Types) -> &mut WalkBuilder {
        self.ig_builder.types(types);
        self
    }

    /// Enables ignoring hidden files.
    ///
    /// This is enabled by default.
    pub fn hidden(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.hidden(yes);
        self
    }

    /// Enables reading ignore files from parent directories.
    ///
    /// If this is enabled, then the parent directories of each file path given
    /// are traversed for ignore files (subject to the ignore settings on
    /// this builder). Note that file paths are canonicalized with respect to
    /// the current working directory in order to determine parent directories.
    ///
    /// This is enabled by default.
    pub fn parents(&mut self, yes: bool) -> &mut WalkBuilder {
        self.parents = yes;
        self
    }

    /// Enables reading `.ignore` files.
    ///
    /// `.ignore` files have the same semantics as `gitignore` files and are
    /// supported by search tools such as ripgrep and The Silver Searcher.
    ///
    /// This is enabled by default.
    pub fn ignore(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.ignore(yes);
        self
    }

    /// Enables reading a global gitignore file, whose path is specified in
    /// git's `core.excludesFile` config option.
    ///
    /// Git's config file location is `$HOME/.gitconfig`. If `$HOME/.gitconfig`
    /// does not exist or does not specify `core.excludesFile`, then
    /// `$XDG_CONFIG_HOME/git/ignore` is read. If `$XDG_CONFIG_HOME` is not
    /// set or is empty, then `$HOME/.config/git/ignore` is used instead.
    pub fn git_global(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_global(yes);
        self
    }

    /// Enables reading `.gitignore` files.
    ///
    /// `.gitignore` files have match semantics as described in the `gitignore`
    /// man page.
    ///
    /// This is enabled by default.
    pub fn git_ignore(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_ignore(yes);
        self
    }

    /// Enables reading `.git/info/exclude` files.
    ///
    /// `.git/info/exclude` files have match semantics as described in the
    /// `gitignore` man page.
    ///
    /// This is enabled by default.
    pub fn git_exclude(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_exclude(yes);
        self
    }
}

/// Walk is a recursive directory iterator over file paths in a directory.
///
/// Only file and directory paths matching the rules are returned. By default,
/// ignore files like `.gitignore` are respected. The precise matching rules
/// and precedence is explained in the documentation for `WalkBuilder`.
pub struct Walk {
    its: vec::IntoIter<(PathBuf, Option<WalkEventIter>)>,
    it: Option<WalkEventIter>,
    ig_root: Ignore,
    ig: Ignore,
    parents: bool,
}

impl Walk {
    /// Creates a new recursive directory iterator for the file path given.
    ///
    /// Note that this uses default settings, which include respecting
    /// `.gitignore` files. To configure the iterator, use `WalkBuilder`
    /// instead.
    pub fn new<P: AsRef<Path>>(path: P) -> Walk {
        WalkBuilder::new(path).build()
    }

    fn skip_entry(&self, ent: &walkdir::DirEntry) -> bool {
        if ent.depth() == 0 {
            // Never skip the root directory.
            return false;
        }
        let m = self.ig.matched(ent.path(), ent.file_type().is_dir());
        if m.is_ignore() {
            debug!("ignoring {}: {:?}", ent.path().display(), m);
            return true;
        } else if m.is_whitelist() {
            debug!("whitelisting {}: {:?}", ent.path().display(), m);
        }
        false
    }
}

impl Iterator for Walk {
    type Item = Result<DirEntry, Error>;

    #[inline(always)]
    fn next(&mut self) -> Option<Result<DirEntry, Error>> {
        loop {
            let ev = match self.it.as_mut().and_then(|it| it.next()) {
                Some(ev) => ev,
                None => {
                    match self.its.next() {
                        None => return None,
                        Some((_, None)) => {
                            return Some(Ok(DirEntry {
                                dent: None,
                                err: None,
                            }));
                        }
                        Some((path, Some(it))) => {
                            self.it = Some(it);
                            if self.parents && path.is_dir() {
                                let (ig, err) = self.ig_root.add_parents(path);
                                self.ig = ig;
                                if let Some(err) = err {
                                    return Some(Err(err));
                                }
                            } else {
                                self.ig = self.ig_root.clone();
                            }
                        }
                    }
                    continue;
                }
            };
            match ev {
                Err(err) => {
                    let path = err.path().map(|p| p.to_path_buf());
                    let mut ig_err = Error::Io(io::Error::from(err));
                    if let Some(path) = path {
                        ig_err = Error::WithPath {
                            path: path.to_path_buf(),
                            err: Box::new(ig_err),
                        };
                    }
                    return Some(Err(ig_err));
                }
                Ok(WalkEvent::Exit) => {
                    self.ig = self.ig.parent().unwrap();
                }
                Ok(WalkEvent::Dir(ent)) => {
                    if self.skip_entry(&ent) {
                        self.it.as_mut().unwrap().it.skip_current_dir();
                        // Still need to push this on the stack because
                        // we'll get a WalkEvent::Exit event for this dir.
                        // We don't care if it errors though.
                        let (igtmp, _) = self.ig.add_child(ent.path());
                        self.ig = igtmp;
                        continue;
                    }
                    let (igtmp, err) = self.ig.add_child(ent.path());
                    self.ig = igtmp;
                    return Some(Ok(DirEntry { dent: Some(ent), err: err }));
                }
                Ok(WalkEvent::File(ent)) => {
                    if self.skip_entry(&ent) {
                        continue;
                    }
                    // If this isn't actually a file (e.g., a symlink),
                    // then skip it.
                    if !ent.file_type().is_file() {
                        continue;
                    }
                    return Some(Ok(DirEntry { dent: Some(ent), err: None }));
                }
            }
        }
    }
}

/// A directory entry with a possible error attached.
///
/// The error typically refers to a problem parsing ignore files in a
/// particular directory.
#[derive(Debug)]
pub struct DirEntry {
    dent: Option<walkdir::DirEntry>,
    err: Option<Error>,
}

impl DirEntry {
    /// The full path that this entry represents.
    pub fn path(&self) -> &Path {
        self.dent.as_ref().map_or(Path::new("<stdin>"), |x| x.path())
    }

    /// Whether this entry corresponds to a symbolic link or not.
    pub fn path_is_symbolic_link(&self) -> bool {
        self.dent.as_ref().map_or(false, |x| x.path_is_symbolic_link())
    }

    /// Returns true if and only if this entry corresponds to stdin.
    ///
    /// i.e., The entry has depth 0 and its file name is `-`.
    pub fn is_stdin(&self) -> bool {
        self.dent.is_none()
    }

    /// Return the metadata for the file that this entry points to.
    pub fn metadata(&self) -> Result<Metadata, Error> {
        if let Some(dent) = self.dent.as_ref() {
            dent.metadata().map_err(|err| Error::WithPath {
                path: self.path().to_path_buf(),
                err: Box::new(Error::Io(io::Error::from(err))),
            })
        } else {
            let ioerr = io::Error::new(
                io::ErrorKind::Other, "stdin has no metadata");
            Err(Error::WithPath {
                path: Path::new("<stdin>").to_path_buf(),
                err: Box::new(Error::Io(ioerr)),
            })
        }
    }

    /// Return the file type for the file that this entry points to.
    ///
    /// This entry doesn't have a file type if it corresponds to stdin.
    pub fn file_type(&self) -> Option<FileType> {
        self.dent.as_ref().map(|x| x.file_type())
    }

    /// Return the file name of this entry.
    ///
    /// If this entry has no file name (e.g., `/`), then the full path is
    /// returned.
    pub fn file_name(&self) -> &OsStr {
        self.dent.as_ref().map_or(OsStr::new("<stdin>"), |x| x.file_name())
    }

    /// Returns the depth at which this entry was created relative to the root.
    pub fn depth(&self) -> usize {
        self.dent.as_ref().map_or(0, |x| x.depth())
    }

    /// Returns an error, if one exists, associated with processing this entry.
    ///
    /// An example of an error is one that occurred while parsing an ignore
    /// file.
    pub fn error(&self) -> Option<&Error> {
        self.err.as_ref()
    }
}

/// WalkEventIter transforms a WalkDir iterator into an iterator that more
/// accurately describes the directory tree. Namely, it emits events that are
/// one of three types: directory, file or "exit." An "exit" event means that
/// the entire contents of a directory have been enumerated.
struct WalkEventIter {
    depth: usize,
    it: walkdir::Iter,
    next: Option<Result<walkdir::DirEntry, walkdir::Error>>,
}

#[derive(Debug)]
enum WalkEvent {
    Dir(walkdir::DirEntry),
    File(walkdir::DirEntry),
    Exit,
}

impl From<WalkDir> for WalkEventIter {
    fn from(it: WalkDir) -> WalkEventIter {
        WalkEventIter { depth: 0, it: it.into_iter(), next: None }
    }
}

impl Iterator for WalkEventIter {
    type Item = walkdir::Result<WalkEvent>;

    #[inline(always)]
    fn next(&mut self) -> Option<walkdir::Result<WalkEvent>> {
        let dent = self.next.take().or_else(|| self.it.next());
        let depth = match dent {
            None => 0,
            Some(Ok(ref dent)) => dent.depth(),
            Some(Err(ref err)) => err.depth(),
        };
        if depth < self.depth {
            self.depth -= 1;
            self.next = dent;
            return Some(Ok(WalkEvent::Exit));
        }
        self.depth = depth;
        match dent {
            None => None,
            Some(Err(err)) => Some(Err(err)),
            Some(Ok(dent)) => {
                if dent.file_type().is_dir() {
                    self.depth += 1;
                    Some(Ok(WalkEvent::Dir(dent)))
                } else {
                    Some(Ok(WalkEvent::File(dent)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;

    use tempdir::TempDir;

    use super::{Walk, WalkBuilder};

    fn wfile<P: AsRef<Path>>(path: P, contents: &str) {
        let mut file = File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    fn mkdirp<P: AsRef<Path>>(path: P) {
        fs::create_dir_all(path).unwrap();
    }

    fn normal_path(unix: &str) -> String {
        if cfg!(windows) {
            unix.replace("\\", "/")
        } else {
            unix.to_string()
        }
    }

    fn walk_collect(prefix: &Path, walk: Walk) -> Vec<String> {
        let mut paths = vec![];
        for dent in walk {
            let dent = dent.unwrap();
            let path = dent.path().strip_prefix(prefix).unwrap();
            if path.as_os_str().is_empty() {
                continue;
            }
            paths.push(normal_path(path.to_str().unwrap()));
        }
        paths.sort();
        paths
    }

    fn mkpaths(paths: &[&str]) -> Vec<String> {
        let mut paths: Vec<_> = paths.iter().map(|s| s.to_string()).collect();
        paths.sort();
        paths
    }

    #[test]
    fn no_ignores() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b/c"));
        mkdirp(td.path().join("x/y"));
        wfile(td.path().join("a/b/foo"), "");
        wfile(td.path().join("x/y/foo"), "");

        let got = walk_collect(td.path(), Walk::new(td.path()));
        assert_eq!(got, mkpaths(&[
            "x", "x/y", "x/y/foo", "a", "a/b", "a/b/foo", "a/b/c",
        ]));
    }

    #[test]
    fn gitignore() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a"));
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let got = walk_collect(td.path(), Walk::new(td.path()));
        assert_eq!(got, mkpaths(&["bar", "a", "a/bar"]));
    }

    #[test]
    fn explicit_ignore() {
        let td = TempDir::new("walk-test-").unwrap();
        let igpath = td.path().join(".not-an-ignore");
        mkdirp(td.path().join("a"));
        wfile(&igpath, "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let mut builder = WalkBuilder::new(td.path());
        assert!(builder.add_ignore(&igpath).is_none());
        let got = walk_collect(td.path(), builder.build());
        assert_eq!(got, mkpaths(&["bar", "a", "a/bar"]));
    }

    #[test]
    fn gitignore_parent() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a"));
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("a/bar"), "");

        let root = td.path().join("a");
        let got = walk_collect(&root, Walk::new(&root));
        assert_eq!(got, mkpaths(&["bar"]));
    }
}
