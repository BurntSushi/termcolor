/*!
The types module provides a way of associating glob patterns on file names to
file types.
*/

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::path::Path;

use gitignore::{self, Gitignore, GitignoreBuilder, Match, Pattern};

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
    ("rr", &["*.R"]),
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
    Gitignore(gitignore::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnrecognizedFileType(_) => "unrecognized file type",
            Error::InvalidDefinition => "invalid definition",
            Error::Gitignore(ref err) => err.description(),
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
            Error::Gitignore(ref err) => err.fmt(f),
        }
    }
}

impl From<gitignore::Error> for Error {
    fn from(err: gitignore::Error) -> Error {
        Error::Gitignore(err)
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
    gi: Option<Gitignore>,
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
    fn new(gi: Option<Gitignore>, has_selected: bool) -> Types {
        Types {
            gi: gi,
            has_selected: has_selected,
            unmatched_pat: Pattern {
                from: Path::new("<filetype>").to_path_buf(),
                original: "<none>".to_string(),
                pat: "<none>".to_string(),
                whitelist: false,
                only_dir: false,
            },
        }
    }

    /// Returns a match for the given path against this file type matcher.
    ///
    /// The path is considered whitelisted if it matches a selected file type.
    /// The path is considered ignored if it matched a negated file type.
    /// If at least one file type is selected and path doesn't match, then
    /// the path is also considered ignored.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match {
        // File types don't apply to directories.
        if is_dir {
            return Match::None;
        }
        let path = path.as_ref();
        self.gi.as_ref()
            .map(|gi| {
                let path = &*path.to_string_lossy();
                let mat = gi.matched_utf8(path, is_dir).invert();
                if self.has_selected && mat.is_none() {
                    Match::Ignored(&self.unmatched_pat)
                } else {
                    mat
                }
            })
            .unwrap_or(Match::None)
    }
}

/// TypesBuilder builds a type matcher from a set of file type definitions and
/// a set of file type selections.
pub struct TypesBuilder {
    types: HashMap<String, Vec<String>>,
    select: Vec<String>,
    select_not: Vec<String>,
}

impl TypesBuilder {
    /// Create a new builder for a file type matcher.
    pub fn new() -> TypesBuilder {
        TypesBuilder {
            types: HashMap::new(),
            select: vec![],
            select_not: vec![],
        }
    }

    /// Build the current set of file type definitions *and* selections into
    /// a file type matcher.
    pub fn build(&self) -> Result<Types, Error> {
        if self.select.is_empty() && self.select_not.is_empty() {
            return Ok(Types::new(None, false));
        }
        let mut bgi = GitignoreBuilder::new("/");
        for name in &self.select {
            let globs = match self.types.get(name) {
                Some(globs) => globs,
                None => {
                    return Err(Error::UnrecognizedFileType(name.to_string()));
                }
            };
            for glob in globs {
                try!(bgi.add("<filetype>", glob));
            }
        }
        for name in &self.select_not {
            let globs = match self.types.get(name) {
                Some(globs) => globs,
                None => {
                    return Err(Error::UnrecognizedFileType(name.to_string()));
                }
            };
            for glob in globs {
                try!(bgi.add("<filetype>", &format!("!{}", glob)));
            }
        }
        Ok(Types::new(Some(try!(bgi.build())), !self.select.is_empty()))
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
    pub fn select(&mut self, name: &str) -> &mut TypesBuilder {
        self.select.push(name.to_string());
        self
    }

    /// Ignore the file type given by `name`.
    pub fn select_not(&mut self, name: &str) -> &mut TypesBuilder {
        self.select_not.push(name.to_string());
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
                    btypes.select_not(selnot);
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
