#![allow(dead_code, unused_variables)]

extern crate docopt;
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
use std::io::{self, BufRead, Write};
use std::process;
use std::result;

use docopt::Docopt;
use regex::bytes::Regex;

use literals::LiteralSets;
use search::{LineSearcher, LineSearcherBuilder};

mod literals;
mod nonl;
mod search;

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
        let expr = try!(parse(&args.arg_pattern));
        let literals = LiteralSets::create(&expr);
        let re = Regex::new(&expr.to_string()).unwrap();
        let _stdin = io::stdin();
        let stdin = _stdin.lock();
        run_by_line(args, &re, stdin)
    } else {
        let searcher =
            try!(LineSearcherBuilder::new(&args.arg_pattern).create());
        run_mmap(args, &searcher)
    }
}

fn run_mmap(args: &Args, searcher: &LineSearcher) -> Result<u64> {
    use memmap::{Mmap, Protection};

    assert!(args.arg_file.len() == 1);
    let mut wtr = io::BufWriter::new(io::stdout());
    let mut count = 0;
    let mmap = try!(Mmap::open_path(&args.arg_file[0], Protection::Read));
    let text = unsafe { mmap.as_slice() };
    for m in searcher.search(text) {
        if !args.flag_count {
            try!(wtr.write(&text[m.start..m.end]));
            try!(wtr.write(b"\n"));
        }
        count += 1;
    }
    if args.flag_count {
        try!(writeln!(wtr, "{}", count));
    }
    Ok(count)
}

fn run_by_line<B: BufRead>(
    args: &Args,
    re: &Regex,
    mut rdr: B,
) -> Result<u64> {
    let mut wtr = io::BufWriter::new(io::stdout());
    let mut count = 0;
    let mut nline = 0;
    let mut line = vec![];
    loop {
        line.clear();
        let n = try!(rdr.read_until(b'\n', &mut line));
        if n == 0 {
            break;
        }
        nline += 1;
        if re.is_match(&line) {
            count += 1;
            try!(wtr.write(&line));
        }
    }
    Ok(count)
}

fn parse(re: &str) -> Result<syntax::Expr> {
    let expr =
        try!(syntax::ExprBuilder::new()
             .allow_bytes(true)
             .unicode(false)
             .parse(re));
    Ok(try!(nonl::remove(expr)))
}
