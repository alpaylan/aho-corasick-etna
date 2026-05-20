//! Fault-localization integration tests for aho-corasick.
//!
//! One `#[test]` per property in src/bin/etna-faultloc.rs's dispatch.

use aho_corasick::etna::{
    property_find_iter_prefilter_parity, property_replace_all_utf8_safe, PropertyResult,
};
use crabcheck::quickcheck::{Arbitrary, Mutate};
use rand::Rng;
use std::fmt;

// ---------- Wrapper newtypes ----------

#[derive(Clone)]
struct BytePatterns(Vec<Vec<u8>>);
#[derive(Clone)]
struct Utf8Haystack(String);
#[derive(Clone)]
struct ShortRepl(String);
#[derive(Clone)]
struct PatternSet(Vec<String>);
#[derive(Clone)]
struct MixHaystack(String);

impl fmt::Debug for BytePatterns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for Utf8Haystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for ShortRepl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for PatternSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for MixHaystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ---------- Generator pools (duplicated from src/bin/etna-faultloc.rs) ----------

const BYTE_POOL: &[u8] = &[
    0x61, 0x62, 0x63, 0x64, 0x78, 0x79, 0x7A, 0x20, b'.', b',', 0xC3, 0xA9, 0xB1, 0xBC, 0xE3,
    0x81, 0x82, 0xF0, 0x9D, 0x84, 0x9E,
];

const HAY_CHARS: &[char] = &[
    'a', 'b', 'c', 'x', 'y', 'z', ' ', '.', ',', 'é', 'ñ', 'ü', 'あ', 'が', '字', '𝄞', '🎉',
];

const REPL_CHARS: &[char] = &['Z', 'Q', '!', ' '];

const PAT_PREFIXES: &[&str] = &["abcd", "mnop", "wxyz", "1234", "pqrs"];
const PAT_SUFFIX_CHARS: &[char] = &['a', 'b', 'c', 'x', 'y', 'z', '0', '1'];

const MAX_PATTERNS: usize = 4;
const MAX_PATTERN_LEN: usize = 4;
const MAX_HAYSTACK_CHARS: usize = 32;
const MAX_REPL_CHARS: usize = 4;

fn random_byte_patterns<R: Rng>(rng: &mut R) -> Vec<Vec<u8>> {
    let n = rng.random_range(1usize..=MAX_PATTERNS);
    (0..n)
        .map(|_| {
            let plen = rng.random_range(1usize..=MAX_PATTERN_LEN);
            (0..plen)
                .map(|_| BYTE_POOL[rng.random_range(0..BYTE_POOL.len())])
                .collect()
        })
        .collect()
}

fn random_utf8_haystack<R: Rng>(rng: &mut R) -> String {
    let len = rng.random_range(0usize..=MAX_HAYSTACK_CHARS);
    (0..len)
        .map(|_| HAY_CHARS[rng.random_range(0..HAY_CHARS.len())])
        .collect()
}

fn random_short_repl<R: Rng>(rng: &mut R) -> String {
    let len = rng.random_range(0usize..=MAX_REPL_CHARS);
    (0..len)
        .map(|_| REPL_CHARS[rng.random_range(0..REPL_CHARS.len())])
        .collect()
}

fn random_pattern_set<R: Rng>(rng: &mut R) -> Vec<String> {
    let n = rng.random_range(4usize..=5);
    let mut patterns: Vec<String> = Vec::with_capacity(n);
    for _ in 0..n {
        let prefix = PAT_PREFIXES[rng.random_range(0..PAT_PREFIXES.len())].to_string();
        let suf_len = rng.random_range(0usize..=3);
        let suffix: String = (0..suf_len)
            .map(|_| PAT_SUFFIX_CHARS[rng.random_range(0..PAT_SUFFIX_CHARS.len())])
            .collect();
        let pat = format!("{prefix}{suffix}");
        if !pat.is_empty() && !patterns.contains(&pat) {
            patterns.push(pat);
        }
    }
    if patterns.is_empty() {
        patterns.push("abcd".to_string());
    }
    let base = patterns[0].clone();
    let extra_len = rng.random_range(1usize..=3);
    let extra: String = (0..extra_len)
        .map(|_| PAT_SUFFIX_CHARS[rng.random_range(0..PAT_SUFFIX_CHARS.len())])
        .collect();
    let superstring = format!("{base}{extra}");
    if !patterns.contains(&superstring) {
        let insert_at = patterns.len().min(1);
        patterns.insert(insert_at, superstring);
    }
    patterns
}

fn random_mix_haystack<R: Rng>(rng: &mut R, patterns: &[String]) -> String {
    let n = rng.random_range(1usize..=6);
    let mut s = String::new();
    for _ in 0..n {
        let choice = rng.random_range(0u8..3);
        match choice {
            0 => {
                if !patterns.is_empty() {
                    let p = &patterns[rng.random_range(0..patterns.len())];
                    s.push_str(p);
                }
            }
            1 => {
                let len = rng.random_range(1usize..=6);
                for _ in 0..len {
                    s.push(PAT_SUFFIX_CHARS[rng.random_range(0..PAT_SUFFIX_CHARS.len())]);
                }
            }
            _ => s.push(' '),
        }
    }
    s
}

// ---------- Arbitrary impls ----------

impl<R: Rng> Arbitrary<R> for BytePatterns {
    fn generate(rng: &mut R, _n: usize) -> Self {
        BytePatterns(random_byte_patterns(rng))
    }
}
impl<R: Rng> Arbitrary<R> for Utf8Haystack {
    fn generate(rng: &mut R, _n: usize) -> Self {
        Utf8Haystack(random_utf8_haystack(rng))
    }
}
impl<R: Rng> Arbitrary<R> for ShortRepl {
    fn generate(rng: &mut R, _n: usize) -> Self {
        ShortRepl(random_short_repl(rng))
    }
}
impl<R: Rng> Arbitrary<R> for PatternSet {
    fn generate(rng: &mut R, _n: usize) -> Self {
        PatternSet(random_pattern_set(rng))
    }
}
impl<R: Rng> Arbitrary<R> for MixHaystack {
    fn generate(rng: &mut R, n: usize) -> Self {
        let ps = PatternSet::generate(rng, n).0;
        MixHaystack(random_mix_haystack(rng, &ps))
    }
}

// ---------- Mutate impls ----------

impl<R: Rng> Mutate<R> for BytePatterns {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        if self.0.is_empty() {
            return BytePatterns(random_byte_patterns(rng));
        }
        let mut v = self.0.clone();
        let pi = rng.random_range(0..v.len());
        if !v[pi].is_empty() {
            let bi = rng.random_range(0..v[pi].len());
            v[pi][bi] = BYTE_POOL[rng.random_range(0..BYTE_POOL.len())];
        }
        BytePatterns(v)
    }
}

impl<R: Rng> Mutate<R> for Utf8Haystack {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut chars: Vec<char> = self.0.chars().collect();
        if chars.is_empty() {
            return Utf8Haystack(random_utf8_haystack(rng));
        }
        let i = rng.random_range(0..chars.len());
        chars[i] = HAY_CHARS[rng.random_range(0..HAY_CHARS.len())];
        Utf8Haystack(chars.into_iter().collect())
    }
}

impl<R: Rng> Mutate<R> for ShortRepl {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut chars: Vec<char> = self.0.chars().collect();
        if chars.is_empty() {
            return ShortRepl(random_short_repl(rng));
        }
        let i = rng.random_range(0..chars.len());
        chars[i] = REPL_CHARS[rng.random_range(0..REPL_CHARS.len())];
        ShortRepl(chars.into_iter().collect())
    }
}

impl<R: Rng> Mutate<R> for PatternSet {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        if self.0.is_empty() {
            return PatternSet(random_pattern_set(rng));
        }
        let mut v = self.0.clone();
        let pi = rng.random_range(0..v.len());
        let mut chars: Vec<char> = v[pi].chars().collect();
        if !chars.is_empty() {
            let ci = rng.random_range(0..chars.len());
            chars[ci] = PAT_SUFFIX_CHARS[rng.random_range(0..PAT_SUFFIX_CHARS.len())];
            let new_s: String = chars.into_iter().collect();
            if !new_s.is_empty() && !v.contains(&new_s) {
                v[pi] = new_s;
            }
        }
        PatternSet(v)
    }
}

impl<R: Rng> Mutate<R> for MixHaystack {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut chars: Vec<char> = self.0.chars().collect();
        if chars.is_empty() {
            return MixHaystack(String::new());
        }
        let i = rng.random_range(0..chars.len());
        let pool: &[char] = &['a', 'b', 'c', 'x', 'y', 'z', '0', '1', ' '];
        chars[i] = pool[rng.random_range(0..pool.len())];
        MixHaystack(chars.into_iter().collect())
    }
}

fn to_opt(r: PropertyResult) -> Option<bool> {
    match r {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

// ---------- Property wrappers (top-level fns the macro can take by name) ----------

fn property_replace_all_utf8_safe_test(
    input: (BytePatterns, Utf8Haystack, ShortRepl),
) -> Option<bool> {
    let (BytePatterns(p), Utf8Haystack(h), ShortRepl(r)) = input;
    to_opt(property_replace_all_utf8_safe(p, h, r))
}

fn property_find_iter_prefilter_parity_test(
    input: (PatternSet, MixHaystack),
) -> Option<bool> {
    let (PatternSet(p), MixHaystack(h)) = input;
    to_opt(property_find_iter_prefilter_parity(p, h))
}

fn property_find_iter_prefilter_parity_no_shrink_test(
    input: (PatternSet, MixHaystack),
) -> Option<bool> {
    let (PatternSet(p), MixHaystack(h)) = input;
    to_opt(property_find_iter_prefilter_parity(p, h))
}

// Manual JSON emitter (we don't depend on serde_json in dev-deps).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{}", x)
    } else {
        "null".to_string()
    }
}

fn emit_locate_json(r: &crabcheck::profiling::LocateResult) {
    use crabcheck::quickcheck::ResultStatus;
    let status = match &r.run.status {
        ResultStatus::Failed { .. } => "Failed",
        ResultStatus::Finished => "Finished",
        ResultStatus::GaveUp => "GaveUp",
        ResultStatus::TimedOut => "TimedOut",
        ResultStatus::Aborted { .. } => "Aborted",
    };
    let top = if let Some(s) = r.top() {
        format!(
            "{{\"rank\":{},\"file\":{},\"function\":{},\"start_line\":{},\"end_line\":{},\"ochiai\":{},\"delta\":{},\"panic_overlap\":{},\"confidence\":{},\"confidence_rule\":{}}}",
            s.rank,
            json_escape(&s.region.file),
            json_escape(&s.region.function),
            s.region.start_line,
            s.region.end_line,
            json_f64(s.region.suspiciousness.ochiai as f64),
            json_f64(s.region.delta as f64),
            s.panic_overlap,
            json_escape(&format!("{}", s.confidence)),
            json_escape(s.confidence_rule),
        )
    } else {
        "null".to_string()
    };
    let top_5_items: Vec<String> = r
        .suspects
        .iter()
        .take(5)
        .map(|s| {
            format!(
                "{{\"rank\":{},\"file\":{},\"function\":{},\"start_line\":{},\"end_line\":{},\"confidence\":{},\"confidence_rule\":{},\"panic_overlap\":{}}}",
                s.rank,
                json_escape(&s.region.file),
                json_escape(&s.region.function),
                s.region.start_line,
                s.region.end_line,
                json_escape(&format!("{}", s.confidence)),
                json_escape(s.confidence_rule),
                s.panic_overlap,
            )
        })
        .collect();
    let top_5 = format!("[{}]", top_5_items.join(","));
    let diag_items: Vec<String> = r.diagnostics.iter().map(|d| json_escape(d.tag())).collect();
    let diags = format!("[{}]", diag_items.join(","));
    let out = format!(
        "{{\"status\":{},\"passed\":{},\"discarded\":{},\"n_panics\":{},\"n_suspects\":{},\"top\":{},\"top_5\":{},\"diagnostics\":{}}}",
        json_escape(status),
        r.run.passed,
        r.run.discarded,
        r.n_panics,
        r.suspects.len(),
        top,
        top_5,
        diags,
    );
    println!("@@LOCATE@@ {}", out);
}

#[test]
fn locate_replace_all_utf8_safe() {
    let report =
        crabcheck::quickcheck_with_locate!(property_replace_all_utf8_safe_test, "aho_corasick");
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_find_iter_prefilter_parity() {
    let report = crabcheck::quickcheck_with_locate!(
        property_find_iter_prefilter_parity_test,
        "aho_corasick"
    );
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_find_iter_prefilter_parity_no_shrink() {
    let report = crabcheck::quickcheck_with_locate!(
        property_find_iter_prefilter_parity_no_shrink_test,
        "aho_corasick"
    );
    eprintln!("{report}");
    emit_locate_json(&report);
}
