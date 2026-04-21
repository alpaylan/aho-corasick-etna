//! ETNA witness tests — deterministic, minimal reproducers per variant.
//!
//! Each `witness_<name>_case_<tag>` test calls `property_<name>` with frozen
//! inputs. On base HEAD every witness must pass; when a variant's patch is
//! applied (or its marauders are activated), at least one witness must fail.

use aho_corasick::etna::{
    property_find_iter_prefilter_parity, property_replace_all_utf8_safe, PropertyResult,
};

fn assert_pass(result: PropertyResult) {
    match result {
        PropertyResult::Pass | PropertyResult::Discard => {}
        PropertyResult::Fail(m) => panic!("property failed: {m}"),
    }
}

// --- replace_all_utf8_safe: e453f60 -------------------------------------

#[test]
fn witness_replace_all_utf8_safe_case_split_codepoint() {
    // Haystack is the multi-byte codepoint "é" (bytes 0xC3 0xA9), followed by
    // "x". Pattern is the single leading byte 0xC3 of "é" — a sub-codepoint
    // slice. Under the fixed `try_replace_all_with`, the match at start=0,
    // end=1 lands on a non-codepoint boundary and is silently skipped. Under
    // the buggy version, `&haystack[0..1]` panics because 1 is not a char
    // boundary of "é".
    let patterns: Vec<Vec<u8>> = vec![vec![0xC3u8]];
    let haystack = "éx".to_string();
    let replacement = "Z".to_string();
    assert_pass(property_replace_all_utf8_safe(
        patterns,
        haystack,
        replacement,
    ));
}

// --- find_iter_prefilter_parity: 2df0983 --------------------------------

#[test]
fn witness_find_iter_prefilter_parity_case_leftmost_prefix_collision() {
    // Four patterns in LeftmostFirst order: "abcd", "abcdxy", "mnop", "wxyz".
    // The second pattern ("abcdxy") is a strict superstring of the first
    // ("abcd"), so its compile walk short-circuits at the "abcd" prefix with
    // `continue 'PATTERNS;` and its internal match state is never added. In
    // the buggy build order, the prefilter's `add` call lives AFTER the
    // `continue`, so the packed prefilter only registers 3 of the 4 patterns
    // ("abcd", "mnop", "wxyz") — their prefilter PatternIDs become 0, 1, 2
    // while the NFA keeps them at 0, 2, 3. When the packed prefilter reports
    // `Candidate::Match`, the returned PatternID is then off-by-one for every
    // pattern after the short-circuited one.
    let patterns: Vec<String> = vec![
        "abcd".into(),
        "abcdxy".into(),
        "mnop".into(),
        "wxyz".into(),
    ];
    let haystack = "abcdxy abcd mnop wxyz".to_string();
    assert_pass(property_find_iter_prefilter_parity(patterns, haystack));
}
