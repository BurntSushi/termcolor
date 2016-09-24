# NAME

rg - recursively search current directory for lines matching a pattern

# SYNOPSIS

rg [*options*] -e PATTERN ... [*<*path*> ...*]

rg [*options*] <*pattern*> [*<*path*> ...*]

rg [*options*] --files [*<*path*> ...*]

rg [*options*] --type-list

rg --help

rg --version

# DESCRIPTION

rg (ripgrep) combines the usability of The Silver Searcher (an ack clone) with
the raw speed of grep.

# COMMON OPTIONS

-a, --text
: Search binary files as if they were text.

-c, --count
: Only show count of line matches for each file.

--color *WHEN*
: Whether to use coloring in match. Valid values are never, always or auto.
  [default: auto]

-e, --regexp *PATTERN* ...
: Use PATTERN to search. This option can be provided multiple times, where all
  patterns given are searched.

-F, --fixed-strings
: Treat the pattern as a literal string instead of a regular expression.

-g, --glob *GLOB* ...
: Include or exclude files for searching that match the given glob. This always
  overrides any other ignore logic. Multiple glob flags may be used. Globbing
  rules match .gitignore globs. Precede a glob with a '!' to exclude it.

-h, --help
: Show this usage message.

-i, --ignore-case
: Case insensitive search.

-n, --line-number
: Show line numbers (1-based). This is enabled by default at a tty.

-N, --no-line-number
: Suppress line numbers.

-q, --quiet
: Do not print anything to stdout.

-r, --replace *ARG*
: Replace every match with the string given. Capture group indices (e.g., $5)
  and names (e.g., $foo) are supported.

-t, --type *TYPE* ...
: Only search files matching TYPE. Multiple type flags may be provided. Use the
  --type-list flag to list all available types.

-T, --type-not *TYPE* ...
: Do not search files matching TYPE. Multiple not-type flags may be provided.

-u, --unrestricted ...
: Reduce the level of 'smart' searching. A single -u doesn't respect .gitignore
  (etc.) files. Two -u flags will search hidden files and directories. Three
  -u flags will search binary files. -uu is equivalent to grep -r, and -uuu is
  equivalent to grep -a -r.

-v, --invert-match
: Invert matching.

-w, --word-regexp
: Only show matches surrounded by word boundaries. This is equivalent to
  putting \\b before and after the search pattern.

# LESS COMMON OPTIONS

-A, --after-context *NUM*
: Show NUM lines after each match.

-B, --before-context *NUM*
: Show NUM lines before each match.

-C, --context *NUM*
: Show NUM lines before and after each match.

--column
: Show column numbers (1 based) in output. This only shows the column
  numbers for the first match on each line. Note that this doesn't try
  to account for Unicode. One byte is equal to one column.

--context-separator *ARG*
: The string to use when separating non-continuous context lines. Escape
  sequences may be used. [default: --]

--debug
: Show debug messages.

--files
: Print each file that would be searched (but don't search).

-H, --with-filename
: Prefix each match with the file name that contains it. This is the
  default when more than one file is searched.

--heading
: Show the file name above clusters of matches from each file.
  This is the default mode at a tty.

--no-heading
: Don't show any file name heading.

--hidden
: Search hidden directories and files. (Hidden directories and files are
  skipped by default.)

-L, --follow
: Follow symlinks.

--mmap
: Search using memory maps when possible. This is enabled by default
  when ripgrep thinks it will be faster. (Note that mmap searching
  doesn't currently support the various context related options.)

--no-mmap
: Never use memory maps, even when they might be faster.

--no-ignore
: Don't respect ignore files (.gitignore, .ignore, etc.)
  This implies --no-ignore-parent.

--no-ignore-parent
: Don't respect ignore files in parent directories.

-p, --pretty
: Alias for --color=always --heading -n.

-j, --threads *ARG*
: The number of threads to use. Defaults to the number of logical CPUs
  (capped at 6). [default: 0]

--version
: Show the version number of ripgrep and exit.

--vimgrep
: Show results with every match on its own line, including line
  numbers and column numbers. (With this option, a line with more
  than one match of the regex will be printed more than once.)

# FILE TYPE MANAGEMENT OPTIONS

--type-list
: Show all supported file types and their associated globs.

--type-add *ARG* ...
: Add a new glob for a particular file type. Note that this must be
  passed to every invocation of rg.

  Example: --type-add html:*.html,*.htm

--type-clear *TYPE* ...
: Clear the file type globs previously defined for TYPE. This only clears
  the default type definitions that are found inside of ripgrep. Note
  that this must be passed to every invocation of rg.
