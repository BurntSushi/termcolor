/*
extern crate ignore;
extern crate walkdir;

use std::env;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

use ignore::ignore::IgnoreBuilder;
use walkdir::WalkDir;

fn main() {
    let path = env::args().nth(1).unwrap();
    let ig = IgnoreBuilder::new().build();
    let wd = WalkDir::new(path);
    let walker = ignore::walk::Iter::new(ig, wd);

    let mut stdout = io::BufWriter::new(io::stdout());
    // let mut count = 0;
    for dirent in walker {
        // count += 1;
        stdout.write(dirent.path().as_os_str().as_bytes()).unwrap();
        stdout.write(b"\n").unwrap();
    }
    // println!("{}", count);
}
*/
fn main() {}
