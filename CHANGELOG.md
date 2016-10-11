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
  Spark,
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
