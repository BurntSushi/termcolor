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

mod glob;

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
        for p in &self.arg_path {
            let mut it = WalkDir::new(p).into_iter();
            loop {
                let ent = match it.next() {
                    None => break,
                    Some(Err(err)) => {
                        eprintln!("{}", err);
                        continue;
                    }
                    Some(Ok(ent)) => ent,
                };
                if is_hidden(&ent) {
                    if ent.file_type().is_dir() {
                        it.skip_current_dir();
                    }
                    continue;
                }
                println!("{}", ent.path().display());
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

fn is_hidden(ent: &walkdir::DirEntry) -> bool {
    ent.depth() > 0 &&
    ent.file_name().to_str().map(|s| s.starts_with(".")).unwrap_or(false)
}
