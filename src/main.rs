#![allow(dead_code, unused_variables)]

extern crate crossbeam;
extern crate docopt;
extern crate env_logger;
extern crate grep;
#[cfg(windows)]
extern crate kernel32;
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memchr;
extern crate memmap;
extern crate num_cpus;
extern crate regex;
extern crate regex_syntax as syntax;
extern crate rustc_serialize;
extern crate term;
extern crate thread_local;
extern crate walkdir;
#[cfg(windows)]
extern crate winapi;

use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;
use std::result;
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam::sync::chase_lev::{self, Steal, Stealer};
use grep::Grep;
use memmap::{Mmap, Protection};
use term::Terminal;
use walkdir::DirEntry;

use args::Args;
use out::Out;
use printer::Printer;
use search::InputBuffer;

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

mod args;
mod gitignore;
mod glob;
mod ignore;
mod out;
mod printer;
mod search;
mod search_buffer;
mod sys;
mod terminal;
mod types;
mod walk;

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

fn main() {
    match Args::parse().and_then(run) {
        Ok(count) if count == 0 => process::exit(1),
        Ok(count) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

fn run(args: Args) -> Result<u64> {
    if args.files() {
        return run_files(args);
    }
    if args.type_list() {
        return run_types(args);
    }
    let args = Arc::new(args);
    let out = Arc::new(Mutex::new(args.out(io::stdout())));
    let mut workers = vec![];

    let mut workq = {
        let (workq, stealer) = chase_lev::deque();
        for _ in 0..args.threads() {
            let worker = Worker {
                args: args.clone(),
                out: out.clone(),
                chan_work: stealer.clone(),
                inpbuf: args.input_buffer(),
                outbuf: Some(vec![]),
                grep: args.grep(),
                match_count: 0,
            };
            workers.push(thread::spawn(move || worker.run()));
        }
        workq
    };
    for p in args.paths() {
        if p == Path::new("-") {
            workq.push(Work::Stdin)
        } else {
            for ent in try!(args.walker(p)) {
                workq.push(Work::File(ent));
            }
        }
    }
    for _ in 0..workers.len() {
        workq.push(Work::Quit);
    }
    let mut match_count = 0;
    for worker in workers {
        match_count += worker.join().unwrap();
    }
    Ok(match_count)
}

fn run_files(args: Args) -> Result<u64> {
    let mut printer = args.printer(io::BufWriter::new(io::stdout()));
    let mut file_count = 0;
    for p in args.paths() {
        if p == Path::new("-") {
            printer.path(&Path::new("<stdin>"));
            file_count += 1;
        } else {
            for ent in try!(args.walker(p)) {
                printer.path(ent.path());
                file_count += 1;
            }
        }
    }
    Ok(file_count)
}

fn run_types(args: Args) -> Result<u64> {
    let mut printer = args.printer(io::BufWriter::new(io::stdout()));
    let mut ty_count = 0;
    for def in args.type_defs() {
        printer.type_def(def);
        ty_count += 1;
    }
    Ok(ty_count)
}

enum Work {
    Stdin,
    File(DirEntry),
    Quit,
}

enum WorkReady {
    Stdin,
    File(DirEntry, File),
}

struct Worker {
    args: Arc<Args>,
    out: Arc<Mutex<Out<io::Stdout>>>,
    chan_work: Stealer<Work>,
    inpbuf: InputBuffer,
    outbuf: Option<Vec<u8>>,
    grep: Grep,
    match_count: u64,
}

impl Worker {
    fn run(mut self) -> u64 {
        self.match_count = 0;
        loop {
            let work = match self.chan_work.steal() {
                Steal::Empty | Steal::Abort => continue,
                Steal::Data(Work::Quit) => break,
                Steal::Data(Work::Stdin) => WorkReady::Stdin,
                Steal::Data(Work::File(ent)) => {
                    match File::open(ent.path()) {
                        Ok(file) => WorkReady::File(ent, file),
                        Err(err) => {
                            eprintln!("{}: {}", ent.path().display(), err);
                            continue;
                        }
                    }
                }
            };
            let mut outbuf = self.outbuf.take().unwrap();
            outbuf.clear();
            let mut printer = self.args.printer(outbuf);
            self.do_work(&mut printer, work);
            let outbuf = printer.into_inner();
            if !outbuf.get_ref().is_empty() {
                let mut out = self.out.lock().unwrap();
                out.write(&outbuf);
            }
            self.outbuf = Some(outbuf.into_inner());
        }
        self.match_count
    }

    fn do_work<W: Send + io::Write>(
        &mut self,
        printer: &mut Printer<W>,
        work: WorkReady,
    ) {
        let result = match work {
            WorkReady::Stdin => {
                let stdin = io::stdin();
                let stdin = stdin.lock();
                self.search(printer, &Path::new("<stdin>"), stdin)
            }
            WorkReady::File(ent, file) => {
                let mut path = ent.path();
                if let Ok(p) = path.strip_prefix("./") {
                    path = p;
                }
                if self.args.mmap() {
                    self.search_mmap(printer, path, &file)
                } else {
                    self.search(printer, path, file)
                }
            }
        };
        match result {
            Ok(count) => {
                self.match_count += count;
            }
            Err(err) => {
                eprintln!("{}", err);
            }
        }
    }

    fn search<R: io::Read, W: Send + io::Write>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        rdr: R,
    ) -> Result<u64> {
        self.args.searcher(
            &mut self.inpbuf,
            printer,
            &self.grep,
            path,
            rdr,
        ).run().map_err(From::from)
    }

    fn search_mmap<W: Send + io::Write>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        file: &File,
    ) -> Result<u64> {
        if try!(file.metadata()).len() == 0 {
            // Opening a memory map with an empty file results in an error.
            return Ok(0);
        }
        let mmap = try!(Mmap::open(file, Protection::Read));
        Ok(self.args.searcher_buffer(
            printer,
            &self.grep,
            path,
            unsafe { mmap.as_slice() },
        ).run())
    }
}
