#![allow(dead_code, unused_variables)]

extern crate docopt;
extern crate grep;
extern crate memchr;
extern crate memmap;
extern crate num_cpus;
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;
extern crate walkdir;

const USAGE: &'static str = "
Usage: xrep [options] <pattern> <path> ...

xrep is like the silver searcher, but faster than it and grep.

At least one path is required. Searching stdin isn't yet supported.

Options:
    -c, --count   Suppress normal output and show count of line matches.
";

use std::error::Error;
use std::io::{self, Write};
use std::process;
use std::result;

use docopt::Docopt;
use grep::Grep;
use walkdir::{WalkDir, WalkDirIterator};

use ignore::Ignore;

macro_rules! errored {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)));
    }
}

macro_rules! eprintln {
    ($($tt:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(&mut ::std::io::stderr(), $($tt)*);
    }}
}

mod gitignore;
mod glob;
mod ignore;

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

#[derive(RustcDecodable)]
struct Args {
    arg_pattern: String,
    arg_path: Vec<String>,
    flag_count: bool,
}

fn main() {
    let args: Args = Docopt::new(USAGE).and_then(|d| d.decode())
                                       .unwrap_or_else(|e| e.exit());
    match args.run() {
        Ok(count) if count == 0 => process::exit(1),
        Ok(_) => process::exit(0),
        Err(err) => {
            let _ = writeln!(&mut io::stderr(), "{}", err);
            process::exit(1);
        }
    }
}

impl Args {
    fn run(&self) -> Result<u64> {
        if self.arg_path.is_empty() {
            return errored!("Searching stdin is not currently supported.");
        }
        let mut stdout = io::BufWriter::new(io::stdout());
        let mut ig = Ignore::new();
        for p in &self.arg_path {
            let mut it = WalkEventIter::from(WalkDir::new(p));
            loop {
                let ev = match it.next() {
                    None => break,
                    Some(Ok(ev)) => ev,
                    Some(Err(err)) => {
                        eprintln!("{}", err);
                        continue;
                    }
                };
                match ev {
                    WalkEvent::Exit => {
                        ig.pop();
                    }
                    WalkEvent::Dir(ent) => {
                        try!(ig.push(ent.path()));
                        if is_hidden(&ent) || ig.ignored(ent.path(), true) {
                        // if is_hidden(&ent) {
                            it.it.skip_current_dir();
                            continue;
                        }
                    }
                    WalkEvent::File(ent) => {
                        if is_hidden(&ent) || ig.ignored(ent.path(), false) {
                        // if is_hidden(&ent) {
                            continue;
                        }
                        let _ = writeln!(
                            &mut stdout, "{}", ent.path().display());
                    }
                }
            }
        }
        Ok(0)
    }

    fn run_mmap_count_only(&self, searcher: &Grep) -> Result<u64> {
        use memmap::{Mmap, Protection};

        assert!(self.arg_path.len() == 1);
        let mut wtr = io::BufWriter::new(io::stdout());
        let mmap = try!(Mmap::open_path(&self.arg_path[0], Protection::Read));
        let text = unsafe { mmap.as_slice() };
        let count = searcher.iter(text).count() as u64;
        try!(writeln!(wtr, "{}", count));
        Ok(count)
    }
}

/// WalkEventIter transforms a WalkDir iterator into an iterator that more
/// accurately describes the directory tree. Namely, it emits events that are
/// one of three types: directory, file or "exit." An "exit" event means that
/// the entire contents of a directory have been enumerated.
struct WalkEventIter {
    depth: usize,
    it: walkdir::Iter,
    next: Option<result::Result<walkdir::DirEntry, walkdir::Error>>,
}

#[derive(Debug)]
enum WalkEvent {
    Dir(walkdir::DirEntry),
    File(walkdir::DirEntry),
    Exit,
}

impl From<walkdir::WalkDir> for WalkEventIter {
    fn from(it: walkdir::WalkDir) -> WalkEventIter {
        WalkEventIter { depth: 0, it: it.into_iter(), next: None }
    }
}

impl Iterator for WalkEventIter {
    type Item = io::Result<WalkEvent>;

    fn next(&mut self) -> Option<io::Result<WalkEvent>> {
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
            Some(Err(err)) => Some(Err(From::from(err))),
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

fn is_hidden(ent: &walkdir::DirEntry) -> bool {
    ent.depth() > 0 &&
    ent.file_name().to_str().map(|s| s.starts_with(".")).unwrap_or(false)
}
