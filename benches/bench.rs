/*!
This module benchmarks the glob implementation. For benchmarks on the ripgrep
tool itself, see the benchsuite directory.
*/
#![feature(test)]

extern crate glob;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate test;

const SHORT: &'static str = "some/needle.txt";
const SHORT_PAT: &'static str = "some/**/needle.txt";

const LONG: &'static str = "some/a/bigger/path/to/the/crazy/needle.txt";
const LONG_PAT: &'static str = "some/**/needle.txt";

#[allow(dead_code, unused_variables)]
#[path = "../src/glob.rs"]
mod reglob;

fn new_glob(pat: &str) -> glob::Pattern {
    glob::Pattern::new(pat).unwrap()
}

fn new_reglob(pat: &str) -> reglob::Set {
    let mut builder = reglob::SetBuilder::new();
    builder.add(pat).unwrap();
    builder.build().unwrap()
}

fn new_reglob_many(pats: &[&str]) -> reglob::Set {
    let mut builder = reglob::SetBuilder::new();
    for pat in pats {
        builder.add(pat).unwrap();
    }
    builder.build().unwrap()
}

#[bench]
fn short_glob(b: &mut test::Bencher) {
    let pat = new_glob(SHORT_PAT);
    b.iter(|| assert!(pat.matches(SHORT)));
}

#[bench]
fn short_regex(b: &mut test::Bencher) {
    let set = new_reglob(SHORT_PAT);
    b.iter(|| assert!(set.is_match(SHORT)));
}

#[bench]
fn long_glob(b: &mut test::Bencher) {
    let pat = new_glob(LONG_PAT);
    b.iter(|| assert!(pat.matches(LONG)));
}

#[bench]
fn long_regex(b: &mut test::Bencher) {
    let set = new_reglob(LONG_PAT);
    b.iter(|| assert!(set.is_match(LONG)));
}

const MANY_SHORT_GLOBS: &'static [&'static str] = &[
    // Taken from a random .gitignore on my system.
    ".*.swp",
    "tags",
    "target",
    "*.lock",
    "tmp",
    "*.csv",
    "*.fst",
    "*-got",
    "*.csv.idx",
    "words",
    "98m*",
    "dict",
    "test",
    "months",
];

const MANY_SHORT_SEARCH: &'static str = "98m-blah.csv.idx";

#[bench]
fn many_short_glob(b: &mut test::Bencher) {
    let pats: Vec<_> = MANY_SHORT_GLOBS.iter().map(|&s| new_glob(s)).collect();
    b.iter(|| {
        let mut count = 0;
        for pat in &pats {
            if pat.matches(MANY_SHORT_SEARCH) {
                count += 1;
            }
        }
        assert_eq!(2, count);
    })
}

#[bench]
fn many_short_regex_set(b: &mut test::Bencher) {
    let set = new_reglob_many(MANY_SHORT_GLOBS);
    b.iter(|| assert_eq!(2, set.matches(MANY_SHORT_SEARCH).iter().count()));
}

// This is the fastest on my system (beating many_glob by about 2x). This
// suggests that a RegexSet needs quite a few regexes (or a larger haystack)
// in order for it to scale.
//
// TODO(burntsushi): come up with a benchmark that uses more complex patterns
// or a longer haystack.
#[bench]
fn many_short_regex_pattern(b: &mut test::Bencher) {
    let pats: Vec<_> = MANY_SHORT_GLOBS.iter().map(|&s| {
        let pat = reglob::Pattern::new(s).unwrap();
        regex::Regex::new(&pat.to_regex()).unwrap()
    }).collect();
    b.iter(|| {
        let mut count = 0;
        for pat in &pats {
            if pat.is_match(MANY_SHORT_SEARCH) {
                count += 1;
            }
        }
        assert_eq!(2, count);
    })
}
