use std::cmp;
use std::iter;
use std::str;

use regex::quote;
use regex::bytes::Regex;
use syntax::{
    Expr, Literals, Lit,
    ByteClass, CharClass, Repeater, ClassRange, ByteRange,
};

#[derive(Debug)]
pub struct LiteralSets {
    prefixes: Literals,
    suffixes: Literals,
    required: Literals,
}

#[derive(Debug)]
pub struct LiteralMatcher {
    re: Regex,
}

impl LiteralSets {
    pub fn create(expr: &Expr) -> Self {
        let mut required = Literals::empty();
        union_required(expr, &mut required);
        LiteralSets {
            prefixes: expr.prefixes(),
            suffixes: expr.suffixes(),
            required: required,
        }
    }

    pub fn to_matcher(&self) -> Option<LiteralMatcher> {
        let pre_lcp = self.prefixes.longest_common_prefix();
        let pre_lcs = self.prefixes.longest_common_suffix();
        let suf_lcp = self.suffixes.longest_common_prefix();
        let suf_lcs = self.suffixes.longest_common_suffix();

        let req_lits = self.required.literals();
        let req = match req_lits.iter().max_by_key(|lit| lit.len()) {
            None => &[],
            Some(req) => &***req,
        };

        let mut lit = pre_lcp;
        if pre_lcs.len() > lit.len() {
            lit = pre_lcs;
        }
        if suf_lcp.len() > lit.len() {
            lit = suf_lcp;
        }
        if suf_lcs.len() > lit.len() {
            lit = suf_lcs;
        }
        if req.len() > lit.len() {
            lit = req;
        }
        if lit.is_empty() {
            None
        } else {
            let s = str::from_utf8(lit).unwrap();
            Some(LiteralMatcher { re: Regex::new(&quote(s)).unwrap() })
        }
    }
}

fn union_required(expr: &Expr, lits: &mut Literals) {
    use syntax::Expr::*;
    match *expr {
        Literal { ref chars, casei: false } => {
            let s: String = chars.iter().cloned().collect();
            lits.cross_add(s.as_bytes());
        }
        Literal { ref chars, casei: true } => {
            for &c in chars {
                let cls = CharClass::new(vec![
                    ClassRange { start: c, end: c },
                ]).case_fold();
                if !lits.add_char_class(&cls) {
                    lits.cut();
                    return;
                }
            }
        }
        LiteralBytes { ref bytes, casei: false } => {
            lits.cross_add(bytes);
        }
        LiteralBytes { ref bytes, casei: true } => {
            for &b in bytes {
                let cls = ByteClass::new(vec![
                    ByteRange { start: b, end: b },
                ]).case_fold();
                if !lits.add_byte_class(&cls) {
                    lits.cut();
                    return;
                }
            }
        }
        Class(ref cls) => {
            if !lits.add_char_class(cls) {
                lits.cut();
            }
        }
        ClassBytes(ref cls) => {
            if !lits.add_byte_class(cls) {
                lits.cut();
            }
        }
        Group { ref e, .. } => {
            union_required(&**e, lits);
        }
        Repeat { ref e, r: Repeater::ZeroOrOne, .. } => lits.cut(),
        Repeat { ref e, r: Repeater::ZeroOrMore, .. } => lits.cut(),
        Repeat { ref e, r: Repeater::OneOrMore, .. } => {
            union_required(&**e, lits);
            lits.cut();
        }
        Repeat { ref e, r: Repeater::Range { min, max }, greedy } => {
            repeat_range_literals(&**e, min, max, greedy, lits, union_required);
        }
        Concat(ref es) if es.is_empty() => {}
        Concat(ref es) if es.len() == 1 => union_required(&es[0], lits),
        Concat(ref es) => {
            for e in es {
                let mut lits2 = lits.to_empty();
                union_required(e, &mut lits2);
                if lits2.is_empty() {
                    lits.cut();
                    continue;
                }
                if lits2.contains_empty() {
                    lits.cut();
                }
                // if !lits.union(lits2) {
                if !lits.cross_product(&lits2) {
                    // If this expression couldn't yield any literal that
                    // could be extended, then we need to quit. Since we're
                    // short-circuiting, we also need to freeze every member.
                    lits.cut();
                    break;
                }
            }
        }
        Alternate(ref es) => {
            alternate_literals(es, lits, union_required);
        }
        _ => lits.cut(),
    }
}

fn repeat_range_literals<F: FnMut(&Expr, &mut Literals)>(
    e: &Expr,
    min: u32,
    max: Option<u32>,
    greedy: bool,
    lits: &mut Literals,
    mut f: F,
) {
    use syntax::Expr::*;

    if min == 0 {
        // This is a bit conservative. If `max` is set, then we could
        // treat this as a finite set of alternations. For now, we
        // just treat it as `e*`.
        lits.cut();
    } else {
        let n = cmp::min(lits.limit_size(), min as usize);
        let es = iter::repeat(e.clone()).take(n).collect();
        f(&Concat(es), lits);
        if n < min as usize {
            lits.cut();
        }
        if max.map_or(true, |max| min < max) {
            lits.cut();
        }
    }
}

fn alternate_literals<F: FnMut(&Expr, &mut Literals)>(
    es: &[Expr],
    lits: &mut Literals,
    mut f: F,
) {
    let mut lits2 = lits.to_empty();
    for e in es {
        let mut lits3 = lits.to_empty();
        lits3.set_limit_size(lits.limit_size() / 5);
        f(e, &mut lits3);
        if lits3.is_empty() || !lits2.union(lits3) {
            // If we couldn't find suffixes for *any* of the
            // alternates, then the entire alternation has to be thrown
            // away and any existing members must be frozen. Similarly,
            // if the union couldn't complete, stop and freeze.
            lits.cut();
            return;
        }
    }
    // All we do at the moment is look for prefixes and suffixes. If both
    // are empty, then we report nothing. We should be able to do better than
    // this, but we'll need something more expressive than just a "set of
    // literals."
    let lcp = lits2.longest_common_prefix();
    let lcs = lits2.longest_common_suffix();
    if !lcp.is_empty() {
        lits.cross_add(lcp);
    }
    lits.cut();
    if !lcs.is_empty() {
        lits.add(Lit::empty());
        lits.add(Lit::new(lcs.to_vec()));
    }
}
