#![allow(dead_code, unused_variables)]

extern crate crossbeam;
extern crate docopt;
extern crate env_logger;
extern crate grep;
#[macro_use]
extern crate log;
extern crate memchr;
extern crate memmap;
extern crate num_cpus;
extern crate parking_lot;
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;
extern crate thread_local;
extern crate walkdir;

use std::cmp;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::result;
use std::sync::Arc;
use std::thread;

use crossbeam::sync::chase_lev::{self, Steal, Stealer};
use docopt::Docopt;
use grep::{Grep, GrepBuilder};
use parking_lot::Mutex;
use walkdir::WalkDir;

use ignore::Ignore;
use printer::Printer;
use search::{InputBuffer, Searcher};

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
mod printer;
mod search;
mod walk;

const USAGE: &'static str = "
Usage: xrep [options] <pattern> [<path> ...]
       xrep --files [<path> ...]

xrep is like the silver searcher and grep, but faster than both.

WARNING: Searching stdin isn't yet supported.

Options:
    -c, --count                Suppress normal output and show count of line
                               matches.
    -A, --after-context NUM    Show NUM lines after each match.
    -B, --before-context NUM   Show NUM lines before each match.
    -C, --context NUM          Show NUM lines before and after each match.
    --debug                    Show debug messages.
    --files                    Print each file that would be searched
                               (but don't search).
    --hidden                   Search hidden directories and files.
    -i, --ignore-case          Case insensitive search.
    -L, --follow               Follow symlinks.
    -n, --line-number          Show line numbers (1-based).
    -t, --threads ARG          The number of threads to use. Defaults to the
                               number of logical CPUs. [default: 0]
    -v, --invert-match         Invert matching.
";

#[derive(RustcDecodable)]
struct Args {
    arg_pattern: String,
    arg_path: Vec<String>,
    flag_after_context: usize,
    flag_before_context: usize,
    flag_context: usize,
    flag_count: bool,
    flag_debug: bool,
    flag_files: bool,
    flag_follow: bool,
    flag_hidden: bool,
    flag_ignore_case: bool,
    flag_invert_match: bool,
    flag_line_number: bool,
    flag_threads: usize,
}

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

fn main() {
    let args: Args = Docopt::new(USAGE).and_then(|d| d.decode())
                                       .unwrap_or_else(|e| e.exit());
    match run(args) {
        Ok(_) => process::exit(0),
        Err(err) => {
            let _ = writeln!(&mut io::stderr(), "{}", err);
            process::exit(1);
        }
    }
}

fn run(mut args: Args) -> Result<()> {
    let mut logb = env_logger::LogBuilder::new();
    if args.flag_debug {
        logb.filter(None, log::LogLevelFilter::Debug);
    } else {
        logb.filter(None, log::LogLevelFilter::Warn);
    }
    if let Err(err) = logb.init() {
        errored!("failed to initialize logger: {}", err);
    }

    if args.arg_path.is_empty() {
        args.arg_path.push("./".to_string());
    }
    if args.arg_path.iter().any(|p| p == "-") {
        errored!("searching <stdin> isn't yet supported");
    }
    if args.flag_files {
        return run_files(args);
    }
    let args = Arc::new(args);
    let mut workers = vec![];
    let stdout = Arc::new(Mutex::new(io::BufWriter::new(io::stdout())));

    let mut chan_work_send = {
        let (worker, stealer) = chase_lev::deque();
        for _ in 0..args.num_workers() {
            let grepb =
                GrepBuilder::new(&args.arg_pattern)
                .case_insensitive(args.flag_ignore_case);
            let worker = Worker {
                args: args.clone(),
                stdout: stdout.clone(),
                chan_work: stealer.clone(),
                inpbuf: InputBuffer::new(),
                outbuf: Some(vec![]),
                grep: try!(grepb.build()),
            };
            workers.push(thread::spawn(move || worker.run()));
        }
        worker
    };

    for p in &args.arg_path {
        for path in args.walker(p) {
            chan_work_send.push(Message::Some(path));
        }
    }
    for _ in 0..workers.len() {
        chan_work_send.push(Message::Quit);
    }
    for worker in workers {
        worker.join().unwrap();
    }
    Ok(())
}

fn run_files(args: Args) -> Result<()> {
    let mut printer = Printer::new(io::BufWriter::new(io::stdout()));
    for p in &args.arg_path {
        for path in args.walker(p) {
            printer.path(path);
        }
    }
    Ok(())
}

impl Args {
    fn printer<W: io::Write>(&self, wtr: W) -> Printer<W> {
        Printer::new(wtr)
    }

    fn num_workers(&self) -> usize {
        let mut num = self.flag_threads;
        if num == 0 {
            num = cmp::min(8, num_cpus::get());
        }
        num
    }

    fn walker<P: AsRef<Path>>(&self, path: P) -> walk::Iter {
        let wd = WalkDir::new(path).follow_links(self.flag_follow);
        let mut ig = Ignore::new();
        ig.ignore_hidden(!self.flag_hidden);
        walk::Iter::new(ig, wd)
    }
}

enum Message<T> {
    Some(T),
    Quit,
}

struct Worker {
    args: Arc<Args>,
    stdout: Arc<Mutex<io::BufWriter<io::Stdout>>>,
    chan_work: Stealer<Message<PathBuf>>,
    inpbuf: InputBuffer,
    outbuf: Option<Vec<u8>>,
    grep: Grep,
}

impl Worker {
    fn run(mut self) {
        loop {
            let path = match self.chan_work.steal() {
                Steal::Empty | Steal::Abort => continue,
                Steal::Data(Message::Quit) => break,
                Steal::Data(Message::Some(path)) => path,
            };
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(err) => {
                    eprintln!("{}: {}", path.display(), err);
                    continue;
                }
            };
            let mut outbuf = self.outbuf.take().unwrap();
            outbuf.clear();
            let mut printer = self.args.printer(outbuf);
            {
                let mut searcher = Searcher::new(
                    &mut self.inpbuf,
                    &mut printer,
                    &self.grep,
                    &path,
                    file,
                );
                searcher = searcher.count(self.args.flag_count);
                searcher = searcher.line_number(self.args.flag_line_number);
                searcher = searcher.invert_match(self.args.flag_invert_match);
                searcher = searcher.before_context(
                    self.args.flag_before_context);
                if let Err(err) = searcher.run() {
                    eprintln!("{}", err);
                }
            }
            let outbuf = printer.into_inner();
            if !outbuf.is_empty() {
                let mut stdout = self.stdout.lock();
                let _ = stdout.write_all(&outbuf);
                let _ = stdout.flush();
            }
            self.outbuf = Some(outbuf);
        }
    }
}
