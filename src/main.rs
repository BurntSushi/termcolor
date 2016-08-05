#![allow(dead_code, unused_variables)]

extern crate docopt;
extern crate grep;
extern crate memchr;
extern crate memmap;
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;

const USAGE: &'static str = "
Usage: rep [options] <pattern> [<file> ...]

Options:
    -c, --count   Suppress normal output and show count of matches.
";

use std::error::Error;
use std::io::{self, Write};
use std::process;
use std::result;

use docopt::Docopt;

use grep::{Grep, GrepBuilder};

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

#[derive(RustcDecodable)]
struct Args {
    arg_pattern: String,
    arg_file: Vec<String>,
    flag_count: bool,
}

fn main() {
    let args = Docopt::new(USAGE).and_then(|d| d.decode())
                                 .unwrap_or_else(|e| e.exit());
    match run(&args) {
        Ok(count) if count == 0 => process::exit(1),
        Ok(_) => process::exit(0),
        Err(err) => {
            let _ = writeln!(&mut io::stderr(), "{}", err);
            process::exit(1);
        }
    }
}

fn run(args: &Args) -> Result<u64> {
    if args.arg_file.is_empty() {
        unimplemented!()
    } else {
        let searcher = try!(GrepBuilder::new(&args.arg_pattern).create());
        if args.flag_count {
            run_mmap_count_only(args, &searcher)
        } else {
            run_mmap(args, &searcher)
        }
    }
}

fn run_mmap(args: &Args, searcher: &Grep) -> Result<u64> {
    unimplemented!()
    /*
    for m in searcher.iter(text) {
        if !args.flag_count {
            try!(wtr.write(&text[m.start()..m.end()]));
            try!(wtr.write(b"\n"));
        }
        count += 1;
    }
    Ok(count)
    */
}

#[inline(never)]
fn run_mmap_count_only(args: &Args, searcher: &Grep) -> Result<u64> {
    use memmap::{Mmap, Protection};

    assert!(args.arg_file.len() == 1);
    let mut wtr = io::BufWriter::new(io::stdout());
    let mmap = try!(Mmap::open_path(&args.arg_file[0], Protection::Read));
    let text = unsafe { mmap.as_slice() };
    let count = searcher.iter(text).count() as u64;
    try!(writeln!(wtr, "{}", count));
    Ok(count)
}
