/*!
The types module provides a way of associating glob patterns on file names to
file types.
*/

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::path::Path;

use regex;

use gitignore::{Match, Pattern};
use glob::{self, MatchOptions};

const TYPE_EXTENSIONS: &'static [(&'static str, &'static [&'static str])] = &[
    ("asm", &["*.asm", "*.s", "*.S"]),
    ("awk", &["*.awk"]),
    ("c", &["*.c", "*.h", "*.H"]),
    ("cbor", &["*.cbor"]),
    ("clojure", &["*.clj", "*.cljs"]),
    ("cmake", &["CMakeLists.txt"]),
    ("coffeescript", &["*.coffee"]),
    ("cpp", &[
        "*.C", "*.cc", "*.cpp", "*.cxx",
        "*.h", "*.H", "*.hh", "*.hpp",
    ]),
    ("csharp", &["*.cs"]),
    ("css", &["*.css"]),
    ("cython", &["*.pyx"]),
    ("dart", &["*.dart"]),
    ("d", &["*.d"]),
    ("elisp", &["*.el"]),
    ("erlang", &["*.erl", "*.hrl"]),
    ("fortran", &[
        "*.f", "*.F", "*.f77", "*.F77", "*.pfo",
        "*.f90", "*.F90", "*.f95", "*.F95",
    ]),
    ("fsharp", &["*.fs", "*.fsx", "*.fsi"]),
    ("go", &["*.go"]),
    ("groovy", &["*.groovy"]),
    ("haskell", &["*.hs", "*.lhs"]),
    ("html", &["*.htm", "*.html"]),
    ("java", &["*.java"]),
    ("js", &["*.js"]),
    ("json", &["*.json"]),
    ("jsonl", &["*.jsonl"]),
    ("lisp", &["*.el", "*.jl", "*.lisp", "*.lsp", "*.sc", "*.scm"]),
    ("lua", &["*.lua"]),
    ("m4", &["*.ac", "*.m4"]),
    ("make", &["gnumakefile", "Gnumakefile", "makefile", "Makefile", "*.mk"]),
    ("markdown", &["*.md"]),
    ("matlab", &["*.m"]),
    ("mk", &["mkfile"]),
    ("ml", &["*.ml"]),
    ("objc", &["*.h", "*.m"]),
    ("objcpp", &["*.h", "*.mm"]),
    ("ocaml", &["*.ml", "*.mli", "*.mll", "*.mly"]),
    ("perl", &["*.perl", "*.pl", "*.PL", "*.plh", "*.plx", "*.pm"]),
    ("php", &["*.php", "*.php3", "*.php4", "*.php5", "*.phtml"]),
    ("py", &["*.py"]),
    ("readme", &["README*", "*README"]),
    ("r", &["*.R", "*.r", "*.Rmd", "*.Rnw"]),
    ("rst", &["*.rst"]),
    ("ruby", &["*.rb"]),
    ("rust", &["*.rs"]),
    ("scala", &["*.scala"]),
    ("sh", &["*.bash", "*.csh", "*.ksh", "*.sh", "*.tcsh"]),
    ("sql", &["*.sql"]),
    ("tex", &["*.tex", "*.cls", "*.sty"]),
    ("txt", &["*.txt"]),
    ("toml", &["*.toml", "Cargo.lock"]),
    ("vala", &["*.vala"]),
    ("vb", &["*.vb"]),
    ("vimscript", &["*.vim"]),
    ("xml", &["*.xml"]),
    ("yacc", &["*.y"]),
    ("yaml", &["*.yaml", "*.yml"]),
];

/// Describes all the possible failure conditions for building a file type
/// matcher.
#[derive(Debug)]
pub enum Error {
    /// We tried to select (or negate) a file type that is not defined.
    UnrecognizedFileType(String),
    /// A user specified file type definition could not be parsed.
    InvalidDefinition,
    /// There was an error building the matcher (probably a bad glob).
    Glob(glob::Error),
    /// There was an error compiling a glob as a regex.
    Regex(regex::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnrecognizedFileType(_) => "unrecognized file type",
            Error::InvalidDefinition => "invalid definition",
            Error::Glob(ref err) => err.description(),
            Error::Regex(ref err) => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnrecognizedFileType(ref ty) => {
                write!(f, "unrecognized file type: {}", ty)
            }
            Error::InvalidDefinition => {
                write!(f, "invalid definition (format is type:glob, e.g., \
                           html:*.html)")
            }
            Error::Glob(ref err) => err.fmt(f),
            Error::Regex(ref err) => err.fmt(f),
        }
    }
}

impl From<glob::Error> for Error {
    fn from(err: glob::Error) -> Error {
        Error::Glob(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::Regex(err)
    }
}

/// A single file type definition.
#[derive(Clone, Debug)]
pub struct FileTypeDef {
    name: String,
    pats: Vec<String>,
}

impl FileTypeDef {
    /// Return the name of this file type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the glob patterns used to recognize this file type.
    pub fn patterns(&self) -> &[String] {
        &self.pats
    }
}

/// Types is a file type matcher.
#[derive(Clone, Debug)]
pub struct Types {
    selected: Option<glob::SetYesNo>,
    negated: Option<glob::SetYesNo>,
    has_selected: bool,
    unmatched_pat: Pattern,
}

impl Types {
    /// Creates a new file type matcher from the given Gitignore matcher. If
    /// not Gitignore matcher is provided, then the file type matcher has no
    /// effect.
    ///
    /// If has_selected is true, then at least one file type was selected.
    /// Therefore, any non-matches should be ignored.
    fn new(
        selected: Option<glob::SetYesNo>,
        negated: Option<glob::SetYesNo>,
        has_selected: bool,
    ) -> Types {
        Types {
            selected: selected,
            negated: negated,
            has_selected: has_selected,
            unmatched_pat: Pattern {
                from: Path::new("<filetype>").to_path_buf(),
                original: "<N/A>".to_string(),
                pat: "<N/A>".to_string(),
                whitelist: false,
                only_dir: false,
            },
        }
    }

    /// Creates a new file type matcher that never matches.
    pub fn empty() -> Types {
        Types::new(None, None, false)
    }

    /// Returns a match for the given path against this file type matcher.
    ///
    /// The path is considered whitelisted if it matches a selected file type.
    /// The path is considered ignored if it matched a negated file type.
    /// If at least one file type is selected and path doesn't match, then
    /// the path is also considered ignored.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match {
        // If we don't have any matcher, then we can't do anything.
        if self.negated.is_none() && self.selected.is_none() {
            return Match::None;
        }
        // File types don't apply to directories.
        if is_dir {
            return Match::None;
        }
        let path = path.as_ref();
        let name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None if self.has_selected => {
                return Match::Ignored(&self.unmatched_pat);
            }
            None => {
                return Match::None;
            }
        };
        if self.negated.as_ref().map(|s| s.is_match(&*name)).unwrap_or(false) {
            return Match::Ignored(&self.unmatched_pat);
        }
        if self.selected.as_ref().map(|s|s.is_match(&*name)).unwrap_or(false) {
            return Match::Whitelist(&self.unmatched_pat);
        }
        if self.has_selected {
            Match::Ignored(&self.unmatched_pat)
        } else {
            Match::None
        }
    }
}

/// TypesBuilder builds a type matcher from a set of file type definitions and
/// a set of file type selections.
pub struct TypesBuilder {
    types: HashMap<String, Vec<String>>,
    selected: Vec<String>,
    negated: Vec<String>,
}

impl TypesBuilder {
    /// Create a new builder for a file type matcher.
    pub fn new() -> TypesBuilder {
        TypesBuilder {
            types: HashMap::new(),
            selected: vec![],
            negated: vec![],
        }
    }

    /// Build the current set of file type definitions *and* selections into
    /// a file type matcher.
    pub fn build(&self) -> Result<Types, Error> {
        let opts = MatchOptions {
            require_literal_separator: true, ..MatchOptions::default()
        };
        let selected_globs =
            if self.selected.is_empty() {
                None
            } else {
                let mut bset = glob::SetBuilder::new();
                for name in &self.selected {
                    let globs = match self.types.get(name) {
                        Some(globs) => globs,
                        None => {
                            let msg = name.to_string();
                            return Err(Error::UnrecognizedFileType(msg));
                        }
                    };
                    for glob in globs {
                        try!(bset.add_with(glob, &opts));
                    }
                }
                Some(try!(bset.build_yesno()))
            };
        let negated_globs =
            if self.negated.is_empty() {
                None
            } else {
                let mut bset = glob::SetBuilder::new();
                for name in &self.negated {
                    let globs = match self.types.get(name) {
                        Some(globs) => globs,
                        None => {
                            let msg = name.to_string();
                            return Err(Error::UnrecognizedFileType(msg));
                        }
                    };
                    for glob in globs {
                        try!(bset.add_with(glob, &opts));
                    }
                }
                Some(try!(bset.build_yesno()))
            };
        Ok(Types::new(
            selected_globs, negated_globs, !self.selected.is_empty()))
    }

    /// Return the set of current file type definitions.
    pub fn definitions(&self) -> Vec<FileTypeDef> {
        let mut defs = vec![];
        for (ref name, ref pats) in &self.types {
            let mut pats = pats.to_vec();
            pats.sort();
            defs.push(FileTypeDef {
                name: name.to_string(),
                pats: pats,
            });
        }
        defs.sort_by(|def1, def2| def1.name().cmp(def2.name()));
        defs
    }

    /// Select the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types are selected.
    pub fn select(&mut self, name: &str) -> &mut TypesBuilder {
        if name == "all" {
            for name in self.types.keys() {
                self.selected.push(name.to_string());
            }
        } else {
            self.selected.push(name.to_string());
        }
        self
    }

    /// Ignore the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types are negated.
    pub fn negate(&mut self, name: &str) -> &mut TypesBuilder {
        if name == "all" {
            for name in self.types.keys() {
                self.negated.push(name.to_string());
            }
        } else {
            self.negated.push(name.to_string());
        }
        self
    }

    /// Clear any file type definitions for the type given.
    pub fn clear(&mut self, name: &str) -> &mut TypesBuilder {
        self.types.remove(name);
        self
    }

    /// Add a new file type definition. `name` can be arbitrary and `pat`
    /// should be a glob recognizing file paths belonging to the `name` type.
    pub fn add(&mut self, name: &str, pat: &str) -> &mut TypesBuilder {
        self.types.entry(name.to_string())
            .or_insert(vec![]).push(pat.to_string());
        self
    }

    /// Add a new file type definition specified in string form. The format
    /// is `name:glob`. Names may not include a colon.
    pub fn add_def(&mut self, def: &str) -> Result<(), Error> {
        let name: String = def.chars().take_while(|&c| c != ':').collect();
        let pat: String = def.chars().skip(name.chars().count() + 1).collect();
        if name.is_empty() || pat.is_empty() {
            return Err(Error::InvalidDefinition);
        }
        self.add(&name, &pat);
        Ok(())
    }

    /// Add a set of default file type definitions.
    pub fn add_defaults(&mut self) -> &mut TypesBuilder {
        for &(name, exts) in TYPE_EXTENSIONS {
            for ext in exts {
                self.add(name, ext);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::TypesBuilder;

    macro_rules! matched {
        ($name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr) => {
            matched!($name, $types, $sel, $selnot, $path, true);
        };
        (not, $name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr) => {
            matched!($name, $types, $sel, $selnot, $path, false);
        };
        ($name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr, $matched:expr) => {
            #[test]
            fn $name() {
                let mut btypes = TypesBuilder::new();
                for tydef in $types {
                    btypes.add_def(tydef).unwrap();
                }
                for sel in $sel {
                    btypes.select(sel);
                }
                for selnot in $selnot {
                    btypes.negate(selnot);
                }
                let types = btypes.build().unwrap();
                let mat = types.matched($path, false);
                assert_eq!($matched, !mat.is_ignored());
            }
        };
    }

    fn types() -> Vec<&'static str> {
        vec![
            "html:*.html",
            "html:*.htm",
            "rust:*.rs",
            "js:*.js",
        ]
    }

    matched!(match1, types(), vec!["rust"], vec![], "lib.rs");
    matched!(match2, types(), vec!["html"], vec![], "index.html");
    matched!(match3, types(), vec!["html"], vec![], "index.htm");
    matched!(match4, types(), vec!["html", "rust"], vec![], "main.rs");
    matched!(match5, types(), vec![], vec![], "index.html");
    matched!(match6, types(), vec![], vec!["rust"], "index.html");

    matched!(not, matchnot1, types(), vec!["rust"], vec![], "index.html");
    matched!(not, matchnot2, types(), vec![], vec!["rust"], "main.rs");
}
