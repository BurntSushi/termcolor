ripgrep (rg)
------------
`ripgrep` is a command line search tool that combines the usability of The
Silver Searcher (an `ack` clone) with the raw speed of GNU grep. `ripgrep` has
first class support on Windows, Mac and Linux, with binary downloads available
for [every release](https://github.com/BurntSushi/ripgrep/releases).

[![Linux build status](https://api.travis-ci.org/BurntSushi/ripgrep.png)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![](http://meritbadge.herokuapp.com/ripgrep)](https://crates.io/crates/ripgrep)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

[![A screenshot of a sample search with ripgrep](http://burntsushi.net/stuff/ripgrep1.png)](http://burntsushi.net/stuff/ripgrep1.png)

### Quick example comparing tools

Search the entire Linux kernel directory (after running `make`) for
`[A-Z]+_SUSPEND`, where all matches must be words.

Please remember that a single benchmark is never enough! Please see my
[blog post on `ripgrep`](http://blog.burntsushi.net/ripgrep/) for a very
detailed comparison with more benchmarks and analysis.

First up, `ripgrep`:

```
$ time rg -n -w '[A-Z]+_SUSPEND' | wc -l
450

real    0m0.245s
user    0m1.647s
sys     0m0.377s
```

Compared with The Silver Searcher:

```
$ time ag -w '[A-Z]+_SUSPEND' | wc -l
450

real    0m0.753s
user    0m2.033s
sys     0m1.673s
```

Or `git grep`:

```
$ time LC_ALL=C git grep -E -n -w '[A-Z]+_SUSPEND' | wc -l
450

real    0m0.823s
user    0m5.253s
sys     0m0.463s
```

Or `git grep` with Unicode enabled (same as `ripgrep` above):

```
$ time LC_ALL=en_US.UTF-8 git grep -E -n -w '[A-Z]+_SUSPEND' | wc -l
450

real    0m2.880s
user    0m19.323s
sys     0m0.350s
```

Or Sift:

```
$ time sift --git -n -w '[A-Z]+_SUSPEND' | wc -l
450

real    0m3.656s
user    0m56.790s
sys     0m0.650s
```

Or The Platinum Searcher:

```
$ time pt -w -e '[A-Z]+_SUSPEND' | wc -l
450

real    0m12.369s
user    1m50.403s
sys     0m13.857s
```

### Why should I use `ripgrep`?

* It can replace both The Silver Searcher and GNU grep because it is faster
  than both. (N.B. It is not, strictly speaking, a "drop-in" replacement for
  both, but the feature sets are far more similar than different.)
* Like The Silver Searcher, `ripgrep` defaults to recursive directory search
  and won't search files ignored by your `.gitignore` files. It also ignores
  hidden and binary files by default. `ripgrep` also implements proper support
  for `.gitignore`, where as there are many bugs related to that functionality
  in The Silver Searcher.
* `ripgrep` can search specific types files. For example, `rg -tpy foo` limits
  your search to Python files and `rg -Tjs foo` excludes Javascript files
  from your search. `ripgrep` can be taught about new file types with custom
  matching rules.
* `ripgrep` supports many features found in `grep`, such as showing the context
  of search results, highlighting matches with color and full Unicode
  support---except `ripgrep` stays fast!

### Is it really faster than everything else?

Yes. A large number of benchmarks with detailed analysis for each is
[available on my blog](http://blog.burntsushi.net/ripgrep/).

Summarizing, `ripgrep` is fast because:

* It is built on top of
  [Rust's regex engine](https://github.com/rust-lang-nursery/regex).
  Rust's regex engine uses finite automata, SIMD and aggressive literal
  optimizations to make searching very fast.
* It supports searching with either memory maps or by searching incrementally
  with an intermediate buffer. The former is better for single files and the
  latter is better for large directories. `ripgrep` chooses the best searching
  strategy for you automatically.
* Applies your ignore patterns in `.gitignore` files using a
  [`RegexSet`](https://doc.rust-lang.org/regex/regex/struct.RegexSet.html).
  That means a single file path can be matched against multiple glob patterns
  simultaneously.
* Uses a Chase-Lev work-stealing queue for quickly distributing work to
  multiple threads.

### Installation

N.B. `ripgrep` is not yet available in any package repositories. I'd like to
fix that in the future.

[Binaries for `ripgrep` are available for Windows, Mac and
Linux.](https://github.com/BurntSushi/ripgrep/releases) Linux binaries are
static executables. Windows binaries are available either as built with MinGW
(GNU) or with Microsoft Visual C++ (MSVC). When possible, prefer MSVC over GNU,
but you'll need to have the
[Microsoft Visual C++ Build
Tools](http://landinghub.visualstudio.com/visual-cpp-build-tools)
installed.

If you're a Rust programmer, `ripgrep` can be installed with `cargo`:

```
$ cargo install ripgrep
```

### Whirlwind tour

The command line usage of `ripgrep` doesn't differ much from other tools that
perform a similar function, so you probably already know how to use `ripgrep`.
The full details can be found in `rg --help`, but let's go on a whirlwind tour.

`ripgrep` detects when its printing to a terminal, and will automatically
colorize your output and show line numbers, just like The Silver Searcher.
Coloring works on Windows too! Colors can be controlled more granularly with
the `--color` flag.

One last thing before we get started: `ripgrep` assumes UTF-8 *everywhere*. It
can still search files that are invalid UTF-8 (like, say, latin-1), but it will
simply not work on UTF-16 encoded files or other more exotic encodings.
[Support for other encodings may
happen.](https://github.com/BurntSushi/ripgrep/issues/1).

To recursively search the current directory, while respecting all `.gitignore`
files:

```
$ rg foobar
```

The above command also respects all `.rgignore` files, including in parent
directories. `.rgignore` files can be used when `.gitignore` files are
insufficient. In all cases, `.rgignore` patterns take precedence over
`.gitignore`.

To ignore all ignore files, use `--no-ignore`:

```
$ rg --no-ignore foobar
```

(Tip: If your ignore files aren't being adhered to like you expect, run your
search with the `--debug` flag.)

Make the search case insensitive with `-i`, invert the search with `-v` or
show the 2 lines before and after every search result with `-C2`.

Force all matches to be surrounded by word boundaries with `-w`.

Search and replace (find first and last names and swap them):

```
$ rg '([A-Z][a-z]+)\s+([A-Z][a-z]+)' --replace '$2, $1'
```

Named groups are supported:

```
$ rg '(?P<first>[A-Z][a-z]+)\s+(?P<last>[A-Z][a-z]+)' --replace '$last, $first'
```

Up the ante with full Unicode support, by matching any uppercase Unicode letter
followed by any sequence of lowercase Unicode letters (good luck doing this
with other search tools!):

```
$ rg '(\p{Lu}\p{Ll}+)\s+(\p{Lu}\p{Ll}+)' --replace '$2, $1'
```

Search only HTML and CSS files:

```
$ rg -thtml -tcss foobar
```

Search everything except for Javascript files:

```
$ rg -Tjs foobar
```

To see a list of types supported, run `rg --type-list`. To add a new type, use
`--type-add`:

```
$ rg --type-add 'foo:*.foo,*.foobar'
```

The type `foo` will now match any file ending with the `.foo` or `.foobar`
extensions.

### Building

`ripgrep` is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/en-US/) in order to compile it.
`ripgrep` compiles with Rust 1.9 (stable) or newer. Building is easy:

```
$ git clone git://github.com/BurntSushi/ripgrep
$ cd ripgrep
$ cargo build --release
$ ./target/release/rg --version
0.1.3
```

If you have a Rust nightly compiler, then you can enable optional SIMD
acceleration like so:

```
RUSTFLAGS="-C target-cpu=native" cargo build --release --features simd-accel
```

### Running tests

`ripgrep` is relatively well tested, including both unit tests and integration
tests. To run the full test suite, use:

```
$ cargo test
```

from the repository root.
