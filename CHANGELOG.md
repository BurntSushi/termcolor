0.4.0
=====
This is a new minor version release of ripgrep that includes a couple very
minor breaking changes, a few new features and lots of bug fixes.

This version of ripgrep upgrades its `regex` dependency from `0.1` to `0.2`,
which includes a few minor syntax changes:

* POSIX character classes now require double bracketing. Previously, the regex
  `[:upper:]` would parse as the `upper` POSIX character class. Now it parses
  as the character class containing the characters `:upper:`. The fix to this
  change is to use `[[:upper:]]` instead. Note that variants like
  `[[:upper:][:blank:]]` continue to work.
* The character `[` must always be escaped inside a character class.
* The characters `&`, `-` and `~` must be escaped if any one of them are
  repeated consecutively. For example, `[&]`, `[\&]`, `[\&\&]`, `[&-&]` are all
  equivalent while `[&&]` is illegal. (The motivation for this and the prior
  change is to provide a backwards compatible path for adding character class
  set notation.)

Feature enhancements:

* Added or improved file type filtering for Crystal, Kotlin, Perl, PowerShell,
  Ruby, Swig
* [FEATURE #83](https://github.com/BurntSushi/ripgrep/issues/83):
  Type definitions can now include other type definitions.
* [FEATURE #243](https://github.com/BurntSushi/ripgrep/issues/243):
  **BREAKING CHANGE**: The `--column` flag now implies `--line-number`.
* [FEATURE #263](https://github.com/BurntSushi/ripgrep/issues/263):
  Add a new `--sort-files` flag.
* [FEATURE #275](https://github.com/BurntSushi/ripgrep/issues/275):
  Add a new `--path-separator` flag. Useful in cygwin.

Bug fixes:

* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Redux: use more portable ANSI color escape sequences when possible.
* [BUG #258](https://github.com/BurntSushi/ripgrep/issues/258):
  Fix bug that caused ripgrep's parallel iterator to spin and burn CPU.
* [BUG #262](https://github.com/BurntSushi/ripgrep/issues/262):
  Document how to install shell completion files.
* [BUG #266](https://github.com/BurntSushi/ripgrep/issues/266),
  [BUG #293](https://github.com/BurntSushi/ripgrep/issues/293):
  Fix handling of bold styling and change the default colors.
* [BUG #268](https://github.com/BurntSushi/ripgrep/issues/268):
  Make lack of backreference support more explicit.
* [BUG #271](https://github.com/BurntSushi/ripgrep/issues/271):
  Remove `~` dependency on clap.
* [BUG #277](https://github.com/BurntSushi/ripgrep/issues/277):
  Fix cosmetic issue in `globset` crate docs.
* [BUG #279](https://github.com/BurntSushi/ripgrep/issues/279):
  ripgrep did not terminate when `-q/--quiet` was given.
* [BUG #281](https://github.com/BurntSushi/ripgrep/issues/281):
  **BREAKING CHANGE**: Completely remove `^C` handling from ripgrep.
* [BUG #284](https://github.com/BurntSushi/ripgrep/issues/284):
  Make docs for `-g/--glob` clearer.
* [BUG #286](https://github.com/BurntSushi/ripgrep/pull/286):
  When stdout is redirected to a file, don't search that file.
* [BUG #287](https://github.com/BurntSushi/ripgrep/pull/287):
  Fix ZSH completions.
* [BUG #295](https://github.com/BurntSushi/ripgrep/pull/295):
  Remove superfluous `memmap` dependency in `grep` crate.
* [BUG #308](https://github.com/BurntSushi/ripgrep/pull/308):
  Improve docs for `-r/--replace`.
* [BUG #313](https://github.com/BurntSushi/ripgrep/pull/313):
  Update bytecount dep to latest version.
* [BUG #318](https://github.com/BurntSushi/ripgrep/pull/318):
  Fix invalid UTF-8 output bug in Windows consoles.


0.3.2
=====
Feature enhancements:

* Added or improved file type filtering for Less, Sass, stylus, Zsh

Bug fixes:

* [BUG #229](https://github.com/BurntSushi/ripgrep/issues/229):
  Make smart case slightly less conservative.
* [BUG #247](https://github.com/BurntSushi/ripgrep/issues/247):
  Clarify use of --heading/--no-heading.
* [BUG #251](https://github.com/BurntSushi/ripgrep/issues/251),
  [BUG #264](https://github.com/BurntSushi/ripgrep/issues/264),
  [BUG #267](https://github.com/BurntSushi/ripgrep/issues/267):
  Fix matching bug caused by literal optimizations.
* [BUG #256](https://github.com/BurntSushi/ripgrep/issues/256):
  Fix bug that caused `rg foo` and `rg foo/` to have different behavior
  when `foo` was a symlink.
* [BUG #270](https://github.com/BurntSushi/ripgrep/issues/270):
  Fix bug where patterns starting with a `-` couldn't be used with the
  `-e/--regexp` flag. (This resolves a regression that was introduced in
  ripgrep 0.3.0.)


0.3.1
=====
Bug fixes:

* [BUG #242](https://github.com/BurntSushi/ripgrep/issues/242):
  ripgrep didn't respect `--colors foo:none` correctly. Now it does.


0.3.0
=====
This is a new minor version release of ripgrep that includes two breaking
changes with lots of bug fixes and some new features and performance
improvements. Notably, if you had a problem with colors or piping on Windows
before, then that should now be fixed in this release.

**BREAKING CHANGES**:

* ripgrep now requires Rust 1.11 to compile. Previously, it could build on
  Rust 1.9. The cause of this was the move from
  [Docopt to Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  for argument parsing.
* The `-e/--regexp` flag can no longer accept a pattern starting with a `-`.
  There are two work-arounds: `rg -- -foo` and `rg [-]foo` or `rg -e [-]foo`
  will all search for the same `-foo` pattern. The cause of this was the move
  from [Docopt to Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  for argument parsing.
  [This may get fixed in the
  future.](https://github.com/kbknapp/clap-rs/issues/742).

Performance improvements:

* [PERF #33](https://github.com/BurntSushi/ripgrep/issues/33):
  ripgrep now performs similar to GNU grep on small corpora.
* [PERF #136](https://github.com/BurntSushi/ripgrep/issues/136):
  ripgrep no longer slows down because of argument parsing when given a large
  argument list.

Feature enhancements:

* Added or improved file type filtering for Elixir.
* [FEATURE #7](https://github.com/BurntSushi/ripgrep/issues/7):
  Add a `-f/--file` flag that causes ripgrep to read patterns from a file.
* [FEATURE #51](https://github.com/BurntSushi/ripgrep/issues/51):
  Add a `--colors` flag that enables one to customize the colors used in
  ripgrep's output.
* [FEATURE #138](https://github.com/BurntSushi/ripgrep/issues/138):
  Add a `--files-without-match` flag that shows only file paths that contain
  zero matches.
* [FEATURE #230](https://github.com/BurntSushi/ripgrep/issues/230):
  Add completion files to the release (Bash, Fish and PowerShell).

Bug fixes:

* [BUG #37](https://github.com/BurntSushi/ripgrep/issues/37):
  Use correct ANSI escape sequences when `TERM=screen.linux`.
* [BUG #94](https://github.com/BurntSushi/ripgrep/issues/94):
  ripgrep now detects stdin on Windows automatically.
* [BUG #117](https://github.com/BurntSushi/ripgrep/issues/117):
  Colors should now work correctly and automatically inside mintty.
* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Colors should now work within Emacs. In particular, `--color=always` will
  emit colors regardless of the current environment.
* [BUG #189](https://github.com/BurntSushi/ripgrep/issues/189):
  Show less content when running `rg -h`. The full help content can be
  accessed with `rg --help`.
* [BUG #210](https://github.com/BurntSushi/ripgrep/issues/210):
  Support non-UTF-8 file names on Unix platforms.
* [BUG #231](https://github.com/BurntSushi/ripgrep/issues/231):
  Switch from block buffering to line buffering.
* [BUG #241](https://github.com/BurntSushi/ripgrep/issues/241):
  Some error messages weren't suppressed when `--no-messages` was used.


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
