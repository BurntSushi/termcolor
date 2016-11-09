0.2.9
=====
Bug fixes:

* [BUG #226](https://github.com/BurntSushi/ripgrep/issues/226):
  File paths explicitly given on the command line weren't searched in parallel.
  (This was a regression in `0.2.7`.)
* [BUG #228](https://github.com/BurntSushi/ripgrep/issues/228):
  If a directory was given to `--ignore-file`, ripgrep's memory usage would
  grow without bound.


0.2.8
=====
Bug fixes:

* Fixed a bug with the SIMD/AVX features for using bytecount in commit
  `4ca15a`.


0.2.7
=====
Performance improvements:

* [PERF #223](https://github.com/BurntSushi/ripgrep/pull/223):
  Added a parallel recursive directory iterator. This results in major
  performance improvements on large repositories.
* [PERF #11](https://github.com/BurntSushi/ripgrep/pull/11):
  ripgrep now uses the `bytecount` library for counting new lines. In some
  cases, ripgrep runs twice as fast. Use
  `RUSTFLAGS="-C target-cpu=native" cargo build --release --features 'simd-accel avx-accel'`
  to get the fastest possible binary.

Feature enhancements:

* Added or improved file type filtering for Agda, Tex, Taskpaper, Markdown,
  asciidoc, textile, rdoc, org, creole, wiki, pod, C#, PDF, C, C++.
* [FEATURE #149](https://github.com/BurntSushi/ripgrep/issues/149):
  Add a new `--no-messages` flag that suppresses error messages.
  Note that `rg foo 2> /dev/null` also works.
* [FEATURE #159](https://github.com/BurntSushi/ripgrep/issues/159):
  Add a new `-m/--max-count` flag that limits the total number of matches
  printed for each file searched.

Bug fixes:

* [BUG #199](https://github.com/BurntSushi/ripgrep/issues/199):
  Fixed a bug where `-S/--smart-case` wasn't being applied correctly to
  literal optimizations.
* [BUG #203](https://github.com/BurntSushi/ripgrep/issues/203):
  Mention the full name, ripgrep, in more places. It now appears in
  the output of `--help` and `--version`. The repository URL is now also
  in the output of `--help` and the man page.
* [BUG #215](https://github.com/BurntSushi/ripgrep/issues/215):
  Include small note about how to search for a pattern that starts with a `-`.


0.2.6
=====
Feature enhancements:

* Added or improved file type filtering for Fish.

Bug fixes:

* [BUG #206](https://github.com/BurntSushi/ripgrep/issues/206):
  Fixed a regression with `-g/--glob` flag in `0.2.5`.


0.2.5
=====
Feature enhancements:

* Added or improved file type filtering for Groovy, Handlebars, Tcl, zsh and
  Python.
* [FEATURE #9](https://github.com/BurntSushi/ripgrep/issues/9):
  Support global gitignore config and `.git/info/exclude` files.
* [FEATURE #45](https://github.com/BurntSushi/ripgrep/issues/45):
  Add --ignore-file flag for specifying additional ignore files.
* [FEATURE #202](https://github.com/BurntSushi/ripgrep/pull/202):
  Introduce a new
  [`ignore`](https://github.com/BurntSushi/ripgrep/tree/master/ignore)
  crate that encapsulates all of ripgrep's gitignore matching logic.

Bug fixes:

* [BUG #44](https://github.com/BurntSushi/ripgrep/issues/44):
  ripgrep runs slowly when given lots of positional arguments that are
  directories.
* [BUG #119](https://github.com/BurntSushi/ripgrep/issues/119):
  ripgrep didn't reset terminal colors if it was interrupted by `^C`.
  Fixed in [PR #187](https://github.com/BurntSushi/ripgrep/pull/187).
* [BUG #184](https://github.com/BurntSushi/ripgrep/issues/184):
  Fixed a bug related to interpreting gitignore files in parent directories.


0.2.4
=====
SKIPPED.


0.2.3
=====
Bug fixes:

* [BUG #164](https://github.com/BurntSushi/ripgrep/issues/164):
  Fixes a segfault on macos builds.
* [BUG #167](https://github.com/BurntSushi/ripgrep/issues/167):
  Clarify documentation for --threads.


0.2.2
=====
Packaging updates:

* `ripgrep` is now in homebrew-core. `brew install ripgrep` will do the trick
  on a Mac.
* `ripgrep` is now in the Archlinux community repository.
  `pacman -S ripgrep` will do the trick on Archlinux.
* Support has been discontinued for i686-darwin.
* Glob matching has been moved out into its own crate:
  [`globset`](https://crates.io/crates/globset).

Feature enhancements:

* Added or improved file type filtering for CMake, config, Jinja, Markdown,
  Spark.
* [FEATURE #109](https://github.com/BurntSushi/ripgrep/issues/109):
  Add a --max-depth flag for directory traversal.
* [FEATURE #124](https://github.com/BurntSushi/ripgrep/issues/124):
  Add -s/--case-sensitive flag. Overrides --smart-case.
* [FEATURE #139](https://github.com/BurntSushi/ripgrep/pull/139):
  The `ripgrep` repo is now a Homebrew tap. This is useful for installing
  SIMD accelerated binaries, which aren't available in homebrew-core.

Bug fixes:

* [BUG #87](https://github.com/BurntSushi/ripgrep/issues/87),
  [BUG #127](https://github.com/BurntSushi/ripgrep/issues/127),
  [BUG #131](https://github.com/BurntSushi/ripgrep/issues/131):
  Various issues related to glob matching.
* [BUG #116](https://github.com/BurntSushi/ripgrep/issues/116):
  --quiet should stop search after first match.
* [BUG #121](https://github.com/BurntSushi/ripgrep/pull/121):
  --color always should show colors, even when --vimgrep is used.
* [BUG #122](https://github.com/BurntSushi/ripgrep/pull/122):
  Colorize file path at beginning of line.
* [BUG #134](https://github.com/BurntSushi/ripgrep/issues/134):
  Processing a large ignore file (thousands of globs) was very slow.
* [BUG #137](https://github.com/BurntSushi/ripgrep/issues/137):
  Always follow symlinks when given as an explicit argument.
* [BUG #147](https://github.com/BurntSushi/ripgrep/issues/147):
  Clarify documentation for --replace.


0.2.1
=====
Feature enhancements:

* Added or improved file type filtering for Clojure and SystemVerilog.
* [FEATURE #89](https://github.com/BurntSushi/ripgrep/issues/89):
  Add a --null flag that outputs a NUL byte after every file path.

Bug fixes:

* [BUG #98](https://github.com/BurntSushi/ripgrep/issues/98):
  Fix a bug in single threaded mode when if opening a file failed, ripgrep
  quit instead of continuing the search.
* [BUG #99](https://github.com/BurntSushi/ripgrep/issues/99):
  Fix another bug in single threaded mode where empty lines were being printed
  by mistake.
* [BUG #105](https://github.com/BurntSushi/ripgrep/issues/105):
  Fix an off-by-one error with --column.
* [BUG #106](https://github.com/BurntSushi/ripgrep/issues/106):
  Fix a bug where a whitespace only line in a gitignore file caused ripgrep
  to panic (i.e., crash).


0.2.0
=====
Feature enhancements:

* Added or improved file type filtering for VB, R, F#, Swift, Nim, Javascript,
  TypeScript
* [FEATURE #20](https://github.com/BurntSushi/ripgrep/issues/20):
  Adds a --no-filename flag.
* [FEATURE #26](https://github.com/BurntSushi/ripgrep/issues/26):
  Adds --files-with-matches flag. Like --count, but only prints file paths
  and doesn't need to count every match.
* [FEATURE #40](https://github.com/BurntSushi/ripgrep/issues/40):
  Switch from using `.rgignore` to `.ignore`. Note that `.rgignore` is
  still supported, but deprecated.
* [FEATURE #68](https://github.com/BurntSushi/ripgrep/issues/68):
  Add --no-ignore-vcs flag that ignores .gitignore but not .ignore.
* [FEATURE #70](https://github.com/BurntSushi/ripgrep/issues/70):
  Add -S/--smart-case flag (but is disabled by default).
* [FEATURE #80](https://github.com/BurntSushi/ripgrep/issues/80):
  Add support for `{foo,bar}` globs.

Many many bug fixes. Thanks every for reporting these and helping make
`ripgrep` better! (Note that I haven't captured every tracking issue here,
some were closed as duplicates.)

* [BUG #8](https://github.com/BurntSushi/ripgrep/issues/8):
  Don't use an intermediate buffer when --threads=1. (Permits constant memory
  usage.)
* [BUG #15](https://github.com/BurntSushi/ripgrep/issues/15):
  Improves the documentation for --type-add.
* [BUG #16](https://github.com/BurntSushi/ripgrep/issues/16),
  [BUG #49](https://github.com/BurntSushi/ripgrep/issues/49),
  [BUG #50](https://github.com/BurntSushi/ripgrep/issues/50),
  [BUG #65](https://github.com/BurntSushi/ripgrep/issues/65):
  Some gitignore globs were being treated as anchored when they weren't.
* [BUG #18](https://github.com/BurntSushi/ripgrep/issues/18):
  --vimgrep reported incorrect column number.
* [BUG #19](https://github.com/BurntSushi/ripgrep/issues/19):
  ripgrep was hanging waiting on stdin in some Windows terminals. Note that
  this introduced a new bug:
  [#94](https://github.com/BurntSushi/ripgrep/issues/94).
* [BUG #21](https://github.com/BurntSushi/ripgrep/issues/21):
  Removes leading `./` when printing file paths.
* [BUG #22](https://github.com/BurntSushi/ripgrep/issues/22):
  Running `rg --help | echo` caused `rg` to panic.
* [BUG #24](https://github.com/BurntSushi/ripgrep/issues/22):
  Clarify the central purpose of rg in its usage message.
* [BUG #25](https://github.com/BurntSushi/ripgrep/issues/25):
  Anchored gitignore globs weren't applied in subdirectories correctly.
* [BUG #30](https://github.com/BurntSushi/ripgrep/issues/30):
  Globs like `foo/**` should match contents of `foo`, but not `foo` itself.
* [BUG #35](https://github.com/BurntSushi/ripgrep/issues/35),
  [BUG #81](https://github.com/BurntSushi/ripgrep/issues/81):
  When automatically detecting stdin, only read if it's a file or a fifo.
  i.e., ignore stdin in `rg foo < /dev/null`.
* [BUG #36](https://github.com/BurntSushi/ripgrep/issues/36):
  Don't automatically pick memory maps on MacOS. Ever.
* [BUG #38](https://github.com/BurntSushi/ripgrep/issues/38):
  Trailing whitespace in gitignore wasn't being ignored.
* [BUG #43](https://github.com/BurntSushi/ripgrep/issues/43):
  --glob didn't work with directories.
* [BUG #46](https://github.com/BurntSushi/ripgrep/issues/46):
  Use one fewer worker thread than what is provided on CLI.
* [BUG #47](https://github.com/BurntSushi/ripgrep/issues/47):
  --help/--version now work even if other options are set.
* [BUG #55](https://github.com/BurntSushi/ripgrep/issues/55):
  ripgrep was refusing to search /proc/cpuinfo. Fixed by disabling memory
  maps for files with zero size.
* [BUG #64](https://github.com/BurntSushi/ripgrep/issues/64):
  The first path given with --files set was ignored.
* [BUG #67](https://github.com/BurntSushi/ripgrep/issues/67):
  Sometimes whitelist globs like `!/dir` weren't interpreted as anchored.
* [BUG #77](https://github.com/BurntSushi/ripgrep/issues/77):
  When -q/--quiet flag was passed, ripgrep kept searching even after a match
  was found.
* [BUG #90](https://github.com/BurntSushi/ripgrep/issues/90):
  Permit whitelisting hidden files.
* [BUG #93](https://github.com/BurntSushi/ripgrep/issues/93):
  ripgrep was extracting an erroneous inner literal from a repeated pattern.
