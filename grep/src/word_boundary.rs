use syntax::Expr;

/// Strips Unicode word boundaries from the given expression.
///
/// The key invariant this maintains is that the expression returned will match
/// *at least* every where the expression given will match. Namely, a match of
/// the returned expression can report false positives but it will never report
/// false negatives.
///
/// If no word boundaries could be stripped, then None is returned.
pub fn strip_unicode_word_boundaries(expr: &Expr) -> Option<Expr> {
    // The real reason we do this is because Unicode word boundaries are the
    // one thing that Rust's regex DFA engine can't handle. When it sees a
    // Unicode word boundary among non-ASCII text, it falls back to one of the
    // slower engines. We work around this limitation by attempting to use
    // a regex to find candidate matches without a Unicode word boundary. We'll
    // only then use the full (and slower) regex to confirm a candidate as a
    // match or not during search.
    use syntax::Expr::*;

    match *expr {
        Concat(ref es) if !es.is_empty() => {
            let first = is_unicode_word_boundary(&es[0]);
            let last = is_unicode_word_boundary(es.last().unwrap());
            // Be careful not to strip word boundaries if there are no other
            // expressions to match.
            match (first, last) {
                (true, false) if es.len() > 1 => {
                    Some(Concat(es[1..].to_vec()))
                }
                (false, true) if es.len() > 1 => {
                    Some(Concat(es[..es.len() - 1].to_vec()))
                }
                (true, true) if es.len() > 2 => {
                    Some(Concat(es[1..es.len() - 1].to_vec()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Returns true if the given expression is a Unicode word boundary.
fn is_unicode_word_boundary(expr: &Expr) -> bool {
    use syntax::Expr::*;

    match *expr {
        WordBoundary => true,
        NotWordBoundary => true,
        Group { ref e, .. } => is_unicode_word_boundary(e),
        _ => false,
    }
}
