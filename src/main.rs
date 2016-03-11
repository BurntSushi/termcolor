#![allow(dead_code, unused_variables)]

extern crate docopt;
extern crate memchr;
extern crate memmap;
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;

const USAGE: &'static str = "
Usage: rep [options] <pattern> [<file> ...]
";

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::process;
use std::result;

use docopt::Docopt;
use regex::bytes::Regex;

mod nonl;

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

#[derive(RustcDecodable)]
struct Args {
    arg_pattern: String,
    arg_file: Vec<String>,
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
    let expr = try!(parse(&args.arg_pattern));
    let re = Regex::new(&expr.to_string()).unwrap();
    if args.arg_file.is_empty() {
        let _stdin = io::stdin();
        let stdin = _stdin.lock();
        run_by_line(args, &re, stdin)
    } else {
        run_mmap(args, &re)
    }
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

fn run_mmap(args: &Args, re: &Regex) -> Result<u64> {
    use memchr::{memchr, memrchr};
    use memmap::{Mmap, Protection};

    assert!(args.arg_file.len() == 1);
    let mut wtr = io::BufWriter::new(io::stdout());
    let mut count = 0;
    let mmap = try!(Mmap::open_path(&args.arg_file[0], Protection::Read));
    let text = unsafe { mmap.as_slice() };
    let mut start = 0;
    while let Some((s, e)) = re.find(&text[start..]) {
        let (s, e) = (start + s, start + e);
        let prevnl = memrchr(b'\n', &text[0..s]).map_or(0, |i| i + 1);
        let nextnl = memchr(b'\n', &text[e..]).map_or(text.len(), |i| e + i);
        try!(wtr.write(&text[prevnl..nextnl]));
        try!(wtr.write(b"\n"));
        start = nextnl + 1;
        count += 1;
        if start >= text.len() {
            break;
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
