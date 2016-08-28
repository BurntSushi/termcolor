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
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;
extern crate walkdir;

use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use std::result;
use std::sync::Arc;
use std::thread;

use crossbeam::sync::{MsQueue, TreiberStack};
use docopt::Docopt;
use grep::{Grep, GrepBuilder};
use walkdir::WalkDir;

use ignore::Ignore;
use printer::Printer;
use search::Searcher;

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

xrep is like the silver searcher and grep, but faster than both.

WARNING: Searching stdin isn't yet supported.

Options:
    -c, --count         Suppress normal output and show count of line matches.
    --debug             Show debug messages.
    --files             Print each file that would be searched
                        (but don't search).
    -L, --follow        Follow symlinks.
    --hidden            Search hidden directories and files.
    -i, --ignore-case   Case insensitive search.
    --threads ARG       The number of threads to use. Defaults to the number
                        of logical CPUs. [default: 0]
";

#[derive(RustcDecodable)]
struct Args {
    arg_pattern: String,
    arg_path: Vec<String>,
    flag_count: bool,
    flag_debug: bool,
    flag_files: bool,
    flag_follow: bool,
    flag_hidden: bool,
    flag_ignore_case: bool,
    flag_threads: usize,
}

impl Args {
    fn printer<W: io::Write>(&self, wtr: W) -> Printer<W> {
        Printer::new(wtr)
    }
}

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

fn main() {
    let args: Args = Docopt::new(USAGE).and_then(|d| d.decode())
                                       .unwrap_or_else(|e| e.exit());
    match real_main(args) {
        Ok(_) => process::exit(0),
        Err(err) => {
            let _ = writeln!(&mut io::stderr(), "{}", err);
            process::exit(1);
        }
    }
}

fn real_main(args: Args) -> Result<()> {
    let mut logb = env_logger::LogBuilder::new();
    if args.flag_debug {
        logb.filter(None, log::LogLevelFilter::Debug);
    } else {
        logb.filter(None, log::LogLevelFilter::Warn);
    }
    if let Err(err) = logb.init() {
        return errored!("failed to initialize logger: {}", err);
    }

    let mut main = Main::new(args);
    try!(main.run_workers());
    let writer = main.run_writer();
    main.scan();
    main.finish_workers();
    main.chan_results.push(Message::Quit);
    writer.join().unwrap();
    Ok(())
}

type ChanWork = Arc<MsQueue<Message<Work>>>;

type ChanResults = Arc<MsQueue<Message<Vec<u8>>>>;

enum Message<T> {
    Some(T),
    Quit,
}

struct Main {
    args: Arc<Args>,
    chan_work: ChanWork,
    chan_results: ChanResults,
    bufs: Arc<Bufs>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl Main {
    fn new(mut args: Args) -> Main {
        if args.arg_path.is_empty() {
            args.arg_path.push("./".to_string());
        }
        Main {
            args: Arc::new(args),
            chan_work: Arc::new(MsQueue::new()),
            chan_results: Arc::new(MsQueue::new()),
            bufs: Arc::new(Bufs::new()),
            workers: vec![],
        }
    }

    fn scan(&mut self) {
        for p in &self.args.arg_path {
            if p == "-" {
                eprintln!("searching <stdin> isn't yet supported");
                continue;
            }
            let wd = WalkDir::new(p).follow_links(self.args.flag_follow);
            let mut ig = Ignore::new();
            ig.ignore_hidden(!self.args.flag_hidden);

            for ent in walk::Iter::new(ig, wd) {
                let mut path = ent.path();
                if let Ok(p) = path.strip_prefix("./") {
                    path = p;
                }
                self.chan_work.push(Message::Some(Work {
                    path: path.to_path_buf(),
                    out: self.bufs.pop(),
                }));
            }
        }
    }

    fn run_writer(&self) -> thread::JoinHandle<()> {
        let wtr = Writer {
            args: self.args.clone(),
            chan_results: self.chan_results.clone(),
            bufs: self.bufs.clone(),
        };
        thread::spawn(move || wtr.run())
    }

    fn run_workers(&mut self) -> Result<()> {
        let mut num = self.args.flag_threads;
        if num == 0 {
            num = num_cpus::get();
        }
        if num < 4 {
            num = 1;
        } else {
            num -= 2;
        }
        println!("running {} workers", num);
        for _ in 0..num {
            try!(self.run_worker());
        }
        Ok(())
    }

    fn run_worker(&mut self) -> Result<()> {
        let grepb =
            GrepBuilder::new(&self.args.arg_pattern)
            .case_insensitive(self.args.flag_ignore_case);
        let worker = Worker {
            args: self.args.clone(),
            chan_work: self.chan_work.clone(),
            chan_results: self.chan_results.clone(),
            grep: try!(grepb.build()),
        };
        self.workers.push(thread::spawn(move || worker.run()));
        Ok(())
    }

    fn finish_workers(&mut self) {
        // We can stop all of the works by sending a quit message.
        // Each worker is guaranteed to receive the quit message exactly
        // once, so we only need to send `self.workers.len()` of them
        for _ in 0..self.workers.len() {
            self.chan_work.push(Message::Quit);
        }
        // Now wait for each to finish.
        while let Some(thread) = self.workers.pop() {
            thread.join().unwrap();
        }
    }
}

struct Writer {
    args: Arc<Args>,
    chan_results: ChanResults,
    bufs: Arc<Bufs>,
}

impl Writer {
    fn run(self) {
        let mut stdout = io::BufWriter::new(io::stdout());
        while let Message::Some(res) = self.chan_results.pop() {
            let _ = stdout.write_all(&res);
            self.bufs.push(res);
        }
    }
}

struct Work {
    path: PathBuf,
    out: Vec<u8>,
}

struct Worker {
    args: Arc<Args>,
    chan_work: ChanWork,
    chan_results: ChanResults,
    grep: Grep,
}

impl Worker {
    fn run(self) {
        while let Message::Some(mut work) = self.chan_work.pop() {
            work.out.clear();
            let printer = self.args.printer(work.out);
            let searcher = Searcher::new(&self.grep, work.path).unwrap();
            let buf = searcher.search(printer);
            self.chan_results.push(Message::Some(buf));
        }
    }
}

/// A pool of buffers used by each worker thread to write matches.
struct Bufs {
    bufs: TreiberStack<Vec<u8>>,
}

impl Bufs {
    pub fn new() -> Bufs {
        Bufs { bufs: TreiberStack::new() }
    }

    pub fn pop(&self) -> Vec<u8> {
        match self.bufs.pop() {
            None => vec![],
            Some(buf) => buf,
        }
    }

    pub fn push(&self, buf: Vec<u8>) {
        self.bufs.push(buf);
    }
}
