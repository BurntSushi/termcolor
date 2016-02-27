#![allow(dead_code)]

extern crate docopt;
extern crate regex;
extern crate rustc_serialize;

const USAGE: &'static str = "
Usage: rep [options] <pattern> [<file> ...]
";

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::process;
use std::result;

use docopt::Docopt;
use regex::internal::{ExecBuilder, Search};

type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

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
    let _stdin = io::stdin();
    let mut rdr = io::BufReader::new(_stdin.lock());
    let mut wtr = io::BufWriter::new(io::stdout());
    let mut count = 0;
    let mut nline = 0;
    let mut line = vec![];
    let re = try!(ExecBuilder::new(&args.arg_pattern).only_utf8(false).build());
    let mut search = Search {
        captures: &mut [],
        matches: &mut [false],
    };
    loop {
        line.clear();
        let n = try!(rdr.read_until(b'\n', &mut line));
        if n == 0 {
            break;
        }
        nline += 1;
        if line.last().map_or(false, |&b| b == b'\n') {
            line.pop().unwrap();
        }
        search.matches[0] = false;
        if re.exec(&mut search, &line, 0) {
            count += 1;
            try!(wtr.write(nline.to_string().as_bytes()));
            try!(wtr.write(&[b':']));
            try!(wtr.write(&line));
            try!(wtr.write(&[b'\n']));
        }
    }
    Ok(count)
}
