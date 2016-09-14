/*!
The walk module implements a recursive directory iterator (using the `walkdir`)
crate that can efficiently skip and ignore files and directories specified in
a user's ignore patterns.
*/

use walkdir::{self, DirEntry, WalkDir, WalkDirIterator};

use ignore::Ignore;

/// Iter is a recursive directory iterator over file paths in a directory.
/// Only file paths should be searched are yielded.
pub struct Iter {
    ig: Ignore,
    it: WalkEventIter,
}

impl Iter {
    /// Create a new recursive directory iterator using the ignore patterns
    /// and walkdir iterator given.
    pub fn new(ig: Ignore, wd: WalkDir) -> Iter {
        Iter {
            ig: ig,
            it: WalkEventIter::from(wd),
        }
    }

    /// Returns true if this entry should be skipped.
    fn skip_entry(&self, ent: &DirEntry) -> bool {
        if ent.depth() == 0 {
            // Never skip the root directory.
            return false;
        }
        if self.ig.ignored(ent.path(), ent.file_type().is_dir()) {
            return true;
        }
        false
    }
}

impl Iterator for Iter {
    type Item = DirEntry;

    fn next(&mut self) -> Option<DirEntry> {
        while let Some(ev) = self.it.next() {
            match ev {
                Err(err) => {
                    eprintln!("{}", err);
                }
                Ok(WalkEvent::Exit) => {
                    self.ig.pop();
                }
                Ok(WalkEvent::Dir(ent)) => {
                    if self.skip_entry(&ent) {
                        self.it.it.skip_current_dir();
                        // Still need to push this on the stack because we'll
                        // get a WalkEvent::Exit event for this dir. We don't
                        // care if it errors though.
                        let _ = self.ig.push(ent.path());
                        continue;
                    }
                    if let Err(err) = self.ig.push(ent.path()) {
                        eprintln!("{}", err);
                        self.it.it.skip_current_dir();
                        continue;
                    }
                }
                Ok(WalkEvent::File(ent)) => {
                    if self.skip_entry(&ent) {
                        continue;
                    }
                    // If this isn't actually a file (e.g., a symlink), then
                    // skip it.
                    if !ent.file_type().is_file() {
                        continue;
                    }
                    return Some(ent);
                }
            }
        }
        None
    }
}

/// WalkEventIter transforms a WalkDir iterator into an iterator that more
/// accurately describes the directory tree. Namely, it emits events that are
/// one of three types: directory, file or "exit." An "exit" event means that
/// the entire contents of a directory have been enumerated.
struct WalkEventIter {
    depth: usize,
    it: walkdir::Iter,
    next: Option<Result<DirEntry, walkdir::Error>>,
}

#[derive(Debug)]
enum WalkEvent {
    Dir(DirEntry),
    File(DirEntry),
    Exit,
}

impl From<WalkDir> for WalkEventIter {
    fn from(it: WalkDir) -> WalkEventIter {
        WalkEventIter { depth: 0, it: it.into_iter(), next: None }
    }
}

impl Iterator for WalkEventIter {
    type Item = walkdir::Result<WalkEvent>;

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
