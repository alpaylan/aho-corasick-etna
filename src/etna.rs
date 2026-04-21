//! ETNA framework-neutral property functions for aho-corasick.
//!
//! Each `property_<name>` is a pure function taking concrete, owned inputs
//! and returning `PropertyResult`. Framework adapters (proptest / quickcheck
//! / crabcheck / hegel) in `src/bin/etna.rs` and witness tests in
//! `tests/etna_witnesses.rs` all call these functions directly — the
//! invariants are never re-implemented inside an adapter.

#![allow(missing_docs)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{AhoCorasick, MatchKind};

pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

// --------------------------------------------------------------------
// Property: replace_all_utf8_safe
//
// Invariant: `AhoCorasick::replace_all(haystack, patterns, replace_with)`
// must never panic and must always produce a valid Rust `String`, even when
// the supplied byte-valued patterns would otherwise match spans that split
// a UTF-8 codepoint in the haystack. The fix in commit e453f60 added an
// `is_char_boundary` guard in `try_replace_all_with` that silently skips
// matches whose endpoints fall inside a multi-byte scalar. The mutation
// removes that guard, so any pattern that matches a partial-codepoint span
// of a non-ASCII haystack panics on the subsequent `&haystack[..m.start()]`
// slice.

pub fn property_replace_all_utf8_safe(
    patterns: Vec<Vec<u8>>,
    haystack: String,
    replacement: String,
) -> PropertyResult {
    // Drop empty or pathologically huge inputs so the property stays fast.
    if patterns.is_empty() || patterns.len() > 8 {
        return PropertyResult::Discard;
    }
    if haystack.len() > 256 || replacement.len() > 16 {
        return PropertyResult::Discard;
    }
    for p in &patterns {
        if p.is_empty() || p.len() > 8 {
            return PropertyResult::Discard;
        }
    }
    let ac = match AhoCorasick::new(&patterns) {
        Ok(ac) => ac,
        Err(_) => return PropertyResult::Discard,
    };
    let reps: Vec<&str> = (0..patterns.len()).map(|_| replacement.as_str()).collect();
    // replace_all takes &str and must never panic or produce invalid UTF-8.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ac.replace_all(&haystack, &reps)
    }));
    match result {
        Ok(out) => {
            // `out` is a Rust `String` — if we got it, UTF-8 is valid. The
            // property is about not panicking. We also sanity-check that the
            // output is a concatenation of plain-text pieces — its length
            // can't exceed the haystack + (replacement growth) times #matches.
            if out.len() > haystack.len() + (patterns.len() + 1) * replacement.len() * 64 {
                return PropertyResult::Fail(format!(
                    "output length {} exceeds sane bound for haystack {:?}",
                    out.len(),
                    haystack
                ));
            }
            PropertyResult::Pass
        }
        Err(_) => PropertyResult::Fail(format!(
            "replace_all panicked on patterns={:?} haystack={:?}",
            patterns, haystack
        )),
    }
}

// --------------------------------------------------------------------
// Property: find_iter_prefilter_parity
//
// Invariant: enabling or disabling the optional prefilter must never change
// the set of matches returned by `AhoCorasick::find_iter` in `LeftmostFirst`
// mode. Commit 2df0983 fixes the noncontiguous NFA compiler so that every
// pattern is registered with the prefilter *at the same point* the NFA
// accepts it, keeping prefilter-pattern-IDs in sync with automaton-pattern-IDs
// in the presence of leftmost-first-prefix short-circuits. Before the fix,
// patterns that the NFA walked into a shared-prefix accept state but whose
// prefilter-registration was skipped caused the prefilter to associate the
// wrong `PatternID` with a prefilter hit, so the find results diverged from
// the prefilter-disabled run.

pub fn property_find_iter_prefilter_parity(
    patterns: Vec<String>,
    haystack: String,
) -> PropertyResult {
    if patterns.is_empty() || patterns.len() > 6 {
        return PropertyResult::Discard;
    }
    if haystack.len() > 256 {
        return PropertyResult::Discard;
    }
    for p in &patterns {
        if p.is_empty() || p.len() > 16 {
            return PropertyResult::Discard;
        }
    }
    let with_pre = match AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .prefilter(true)
        .build(&patterns)
    {
        Ok(ac) => ac,
        Err(_) => return PropertyResult::Discard,
    };
    let without_pre = match AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .prefilter(false)
        .build(&patterns)
    {
        Ok(ac) => ac,
        Err(_) => return PropertyResult::Discard,
    };
    let a: Vec<(usize, usize, usize)> = with_pre
        .find_iter(&haystack)
        .map(|m| (m.pattern().as_usize(), m.start(), m.end()))
        .collect();
    let b: Vec<(usize, usize, usize)> = without_pre
        .find_iter(&haystack)
        .map(|m| (m.pattern().as_usize(), m.start(), m.end()))
        .collect();
    if a != b {
        return PropertyResult::Fail(format!(
            "prefilter divergence: with_pre={:?}, without_pre={:?}",
            a, b
        ));
    }
    PropertyResult::Pass
}
