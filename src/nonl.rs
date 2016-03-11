use syntax::Expr;

use Result;

/// Returns a new expression that is guaranteed to never match `\n`.
///
/// If the expression contains a literal `\n`, then an error is returned.
pub fn remove(expr: Expr) -> Result<Expr> {
    use syntax::Expr::*;
    Ok(match expr {
        Literal { chars, casei } => {
            if chars.iter().position(|&c| c == '\n').is_some() {
                return Err(format!("Literal '\\n' are not allowed.").into());
            }
            Literal { chars: chars, casei: casei }
        }
        LiteralBytes { bytes, casei } => {
            if bytes.iter().position(|&b| b == b'\n').is_some() {
                return Err(format!("Literal '\\n' are not allowed.").into());
            }
            LiteralBytes { bytes: bytes, casei: casei }
        }
        AnyChar => AnyCharNoNL,
        AnyByte => AnyByteNoNL,
        Class(mut cls) => {
            cls.remove('\n');
            Class(cls)
        }
        ClassBytes(mut cls) => {
            cls.remove(b'\n');
            ClassBytes(cls)
        }
        Group { e, i, name } => {
            Group {
                e: Box::new(try!(remove(*e))),
                i: i,
                name: name,
            }
        }
        Repeat { e, r, greedy } => {
            Repeat {
                e: Box::new(try!(remove(*e))),
                r: r,
                greedy: greedy,
            }
        }
        Concat(exprs) => {
            Concat(try!(exprs.into_iter().map(remove).collect()))
        }
        Alternate(exprs) => {
            Alternate(try!(exprs.into_iter().map(remove).collect()))
        }
        e => e,
    })
}
