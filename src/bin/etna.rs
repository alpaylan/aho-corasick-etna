// ETNA workload runner for aho-corasick.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: ReplaceAllUtf8Safe | FindIterPrefilterParity | All
//
// Every invocation prints exactly one JSON line to stdout and exits 0
// (except argv parsing, which exits 2).

use aho_corasick::etna::{
    property_find_iter_prefilter_parity, property_replace_all_utf8_safe, PropertyResult,
};
use crabcheck::quickcheck as crabcheck_qc;
use crabcheck::quickcheck::Arbitrary as CcArbitrary;
use hegel::{generators as hgen, HealthCheck, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestError, TestRunner};
use quickcheck::{Arbitrary as QcArbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use rand::Rng;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &["ReplaceAllUtf8Safe", "FindIterPrefilterParity"];

fn cases_budget() -> u64 {
    std::env::var("ETNA_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(u64::MAX)
}

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if let Err(e) = r {
            return (Err(e), total);
        }
    }
    (Ok(()), total)
}

// ---------- Canonical witness builders ----------

fn canonical_replace_all_utf8_safe() -> (Vec<Vec<u8>>, String, String) {
    // Pattern is the first byte of "é" (0xC3); haystack contains the
    // multi-byte scalar. The boundary guard must keep replace_all from
    // slicing inside the codepoint. See tests/etna_witnesses.rs for prose.
    (vec![vec![0xC3u8]], "éx".to_string(), "Z".to_string())
}

fn canonical_find_iter_prefilter_parity() -> (Vec<String>, String) {
    // Prefix-collision patterns that force the packed/Teddy prefilter to
    // drop the superstring of "abcd" via leftmost-first short-circuit.
    (
        vec![
            "abcd".into(),
            "abcdxy".into(),
            "mnop".into(),
            "wxyz".into(),
        ],
        "abcdxy abcd mnop wxyz".to_string(),
    )
}

fn check_replace_all_utf8_safe() -> Result<(), String> {
    let (p, h, r) = canonical_replace_all_utf8_safe();
    to_err(property_replace_all_utf8_safe(p, h, r))
}

fn check_find_iter_prefilter_parity() -> Result<(), String> {
    let (p, h) = canonical_find_iter_prefilter_parity();
    to_err(property_find_iter_prefilter_parity(p, h))
}

// ---------- etna (deterministic witness replay) ----------

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        "ReplaceAllUtf8Safe" => check_replace_all_utf8_safe(),
        "FindIterPrefilterParity" => check_find_iter_prefilter_parity(),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            );
        }
    };
    (
        result,
        Metrics {
            inputs: 1,
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ---------- shared Arbitrary-biased generators (qc + cc) ----------
//
// ReplaceAllUtf8Safe: `BytePatterns` is a small list of short byte strings
// where every byte is drawn from a pool of UTF-8 start/continuation bytes
// (so sub-codepoint slices are frequent). `Utf8Haystack` is a short string
// with heavy non-ASCII content. `ShortRepl` is a short ASCII replacement.
//
// FindIterPrefilterParity: `PatternSet` biases toward patterns with shared
// 4-byte prefixes (triggering the Teddy/packed prefilter) and occasional
// superstrings of an earlier pattern. `MixHaystack` concatenates random
// snippets and literal pattern fragments.

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
impl fmt::Display for BytePatterns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for Utf8Haystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for ShortRepl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for PatternSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for MixHaystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

// Byte pool: ASCII printable + a handful of UTF-8 start / continuation
// bytes so single-byte patterns land inside multi-byte codepoints.
const BYTE_POOL: &[u8] = &[
    0x61, 0x62, 0x63, 0x64, 0x78, 0x79, 0x7A, 0x20, b'.', b',', 0xC3, 0xA9, 0xB1, 0xBC, 0xE3,
    0x81, 0x82, 0xF0, 0x9D, 0x84, 0x9E,
];

// Haystack char pool heavy on multi-byte scalars.
const HAY_CHARS: &[char] = &[
    'a', 'b', 'c', 'x', 'y', 'z', ' ', '.', ',', 'é', 'ñ', 'ü', 'あ', 'が', '字', '𝄞', '🎉',
];

// Replacement pool — plain ASCII keeps length math tight.
const REPL_CHARS: &[char] = &['Z', 'Q', '!', ' '];

// Prefix alphabet for pattern-set generator (kept small so patterns share
// 4-byte prefixes often enough to trigger the packed prefilter).
const PAT_PREFIXES: &[&str] = &["abcd", "mnop", "wxyz", "1234", "pqrs"];
const PAT_SUFFIX_CHARS: &[char] = &['a', 'b', 'c', 'x', 'y', 'z', '0', '1'];

fn random_byte_patterns<R: Rng>(rng: &mut R) -> Vec<Vec<u8>> {
    let n = rng.random_range(1usize..=4);
    (0..n)
        .map(|_| {
            let plen = rng.random_range(1usize..=4);
            (0..plen)
                .map(|_| BYTE_POOL[rng.random_range(0..BYTE_POOL.len())])
                .collect()
        })
        .collect()
}

fn random_utf8_haystack<R: Rng>(rng: &mut R) -> String {
    let len = rng.random_range(0usize..=32);
    (0..len)
        .map(|_| HAY_CHARS[rng.random_range(0..HAY_CHARS.len())])
        .collect()
}

fn random_short_repl<R: Rng>(rng: &mut R) -> String {
    let len = rng.random_range(0usize..=4);
    (0..len)
        .map(|_| REPL_CHARS[rng.random_range(0..REPL_CHARS.len())])
        .collect()
}

fn random_pattern_set<R: Rng>(rng: &mut R) -> Vec<String> {
    // See `pattern_set_strategy` for why we force ≥4 patterns and always
    // append a superstring of an existing pattern.
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
    // Build a haystack that concatenates literal pattern fragments and
    // filler so the prefilter actually fires.
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

impl QcArbitrary for BytePatterns {
    fn arbitrary(g: &mut Gen) -> Self {
        let n = g.random_range(1usize..=4);
        let ps: Vec<Vec<u8>> = (0..n)
            .map(|_| {
                let plen = g.random_range(1usize..=4);
                (0..plen)
                    .map(|_| BYTE_POOL[g.random_range(0..BYTE_POOL.len())])
                    .collect()
            })
            .collect();
        BytePatterns(ps)
    }
}

impl QcArbitrary for Utf8Haystack {
    fn arbitrary(g: &mut Gen) -> Self {
        let len = g.random_range(0usize..=32);
        let s: String = (0..len)
            .map(|_| HAY_CHARS[g.random_range(0..HAY_CHARS.len())])
            .collect();
        Utf8Haystack(s)
    }
}

impl QcArbitrary for ShortRepl {
    fn arbitrary(g: &mut Gen) -> Self {
        let len = g.random_range(0usize..=4);
        let s: String = (0..len)
            .map(|_| REPL_CHARS[g.random_range(0..REPL_CHARS.len())])
            .collect();
        ShortRepl(s)
    }
}

impl QcArbitrary for PatternSet {
    fn arbitrary(g: &mut Gen) -> Self {
        let n = g.random_range(4usize..=5);
        let mut patterns: Vec<String> = Vec::with_capacity(n);
        for _ in 0..n {
            let prefix = PAT_PREFIXES[g.random_range(0..PAT_PREFIXES.len())].to_string();
            let suf_len = g.random_range(0usize..=3);
            let suffix: String = (0..suf_len)
                .map(|_| PAT_SUFFIX_CHARS[g.random_range(0..PAT_SUFFIX_CHARS.len())])
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
        let extra_len = g.random_range(1usize..=3);
        let extra: String = (0..extra_len)
            .map(|_| PAT_SUFFIX_CHARS[g.random_range(0..PAT_SUFFIX_CHARS.len())])
            .collect();
        let superstring = format!("{base}{extra}");
        if !patterns.contains(&superstring) {
            let insert_at = patterns.len().min(1);
            patterns.insert(insert_at, superstring);
        }
        PatternSet(patterns)
    }
}

impl QcArbitrary for MixHaystack {
    fn arbitrary(g: &mut Gen) -> Self {
        // `Gen` doesn't thread an outer pattern-set through. Draw a fresh
        // pattern set here purely for haystack priming, then discard it.
        let ps = PatternSet::arbitrary(g).0;
        let n = g.random_range(1usize..=6);
        let mut s = String::new();
        for _ in 0..n {
            let choice = g.random_range(0u8..3);
            match choice {
                0 => {
                    if !ps.is_empty() {
                        let p = &ps[g.random_range(0..ps.len())];
                        s.push_str(p);
                    }
                }
                1 => {
                    let len = g.random_range(1usize..=6);
                    for _ in 0..len {
                        s.push(PAT_SUFFIX_CHARS[g.random_range(0..PAT_SUFFIX_CHARS.len())]);
                    }
                }
                _ => s.push(' '),
            }
        }
        MixHaystack(s)
    }
}

impl<R: Rng> CcArbitrary<R> for BytePatterns {
    fn generate(rng: &mut R, _n: usize) -> Self {
        BytePatterns(random_byte_patterns(rng))
    }
}
impl<R: Rng> CcArbitrary<R> for Utf8Haystack {
    fn generate(rng: &mut R, _n: usize) -> Self {
        Utf8Haystack(random_utf8_haystack(rng))
    }
}
impl<R: Rng> CcArbitrary<R> for ShortRepl {
    fn generate(rng: &mut R, _n: usize) -> Self {
        ShortRepl(random_short_repl(rng))
    }
}
impl<R: Rng> CcArbitrary<R> for PatternSet {
    fn generate(rng: &mut R, _n: usize) -> Self {
        PatternSet(random_pattern_set(rng))
    }
}
impl<R: Rng> CcArbitrary<R> for MixHaystack {
    fn generate(rng: &mut R, n: usize) -> Self {
        let ps = PatternSet::generate(rng, n).0;
        MixHaystack(random_mix_haystack(rng, &ps))
    }
}

// ---------- proptest ----------

fn byte_patterns_strategy() -> BoxedStrategy<Vec<Vec<u8>>> {
    prop::collection::vec(
        prop::collection::vec(prop::sample::select(BYTE_POOL.to_vec()), 1..=4),
        1..=4,
    )
    .boxed()
}

fn utf8_haystack_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(prop::sample::select(HAY_CHARS.to_vec()), 0..=32)
        .prop_map(|cs: Vec<char>| cs.into_iter().collect())
        .boxed()
}

fn short_repl_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(prop::sample::select(REPL_CHARS.to_vec()), 0..=4)
        .prop_map(|cs: Vec<char>| cs.into_iter().collect())
        .boxed()
}

fn pattern_set_strategy() -> BoxedStrategy<Vec<String>> {
    // The prefilter-pattern-ID bug needs ≥4 patterns (to arm the packed
    // prefilter), at least one pattern that is a strict superstring of an
    // earlier pattern (to trigger the leftmost-first short-circuit), AND
    // at least one pattern *after* the superstring in the list (otherwise
    // no pattern ID ends up out-of-sync). We guarantee all three by
    // generating a base list of 4..=5 prefix+suffix patterns, then
    // inserting a superstring at position 1 so the remaining patterns
    // shift down in the prefilter's pattern-ID space.
    (
        prop::collection::vec(
            (
                prop::sample::select(PAT_PREFIXES.to_vec()),
                prop::collection::vec(prop::sample::select(PAT_SUFFIX_CHARS.to_vec()), 0..=3),
            ),
            4..=5,
        ),
        prop::collection::vec(prop::sample::select(PAT_SUFFIX_CHARS.to_vec()), 1..=3),
    )
        .prop_map(|(parts, extra): (Vec<(&'static str, Vec<char>)>, Vec<char>)| {
            let mut out: Vec<String> = Vec::new();
            for (prefix, suffix) in parts {
                let suf: String = suffix.into_iter().collect();
                let p = format!("{prefix}{suf}");
                if !p.is_empty() && !out.contains(&p) {
                    out.push(p);
                }
            }
            if out.is_empty() {
                out.push("abcd".to_string());
            }
            let base = out[0].clone();
            let suffix: String = extra.into_iter().collect();
            let superstring = format!("{base}{suffix}");
            if !out.contains(&superstring) {
                let insert_at = out.len().min(1);
                out.insert(insert_at, superstring);
            }
            out
        })
        .boxed()
}

fn mix_haystack_strategy() -> BoxedStrategy<(Vec<String>, String)> {
    (
        pattern_set_strategy(),
        prop::collection::vec(
            (
                0u8..3,
                0usize..64,
                prop::collection::vec(prop::sample::select(PAT_SUFFIX_CHARS.to_vec()), 1..=6),
            ),
            2..=8,
        ),
    )
        .prop_map(|(ps, segs)| {
            let mut s = String::new();
            for (choice, pick, filler) in segs {
                match choice {
                    0 => {
                        if !ps.is_empty() {
                            let idx = pick % ps.len();
                            s.push_str(&ps[idx]);
                        }
                    }
                    1 => {
                        for c in filler {
                            s.push(c);
                        }
                    }
                    _ => s.push(' '),
                }
            }
            (ps, s)
        })
        .boxed()
}

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let cfg = ProptestConfig {
        cases: cases_budget() as u32,
        max_shrink_iters: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(cfg);
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "ReplaceAllUtf8Safe" => runner
            .run(
                &(
                    byte_patterns_strategy(),
                    utf8_haystack_strategy(),
                    short_repl_strategy(),
                ),
                move |(patterns, haystack, replacement)| {
                    c.fetch_add(1, Ordering::Relaxed);
                    let cex_p = patterns.clone();
                    let cex_h = haystack.clone();
                    let cex_r = replacement.clone();
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        property_replace_all_utf8_safe(patterns, haystack, replacement)
                    }));
                    match outcome {
                        Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                        Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(format!(
                            "({:?} {:?} {:?})",
                            cex_p, cex_h, cex_r
                        ))),
                    }
                },
            )
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "FindIterPrefilterParity" => runner
            .run(&mix_haystack_strategy(), move |(patterns, haystack)| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex_p = patterns.clone();
                let cex_h = haystack.clone();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_find_iter_prefilter_parity(patterns, haystack)
                }));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?} {:?})", cex_p, cex_h)))
                    }
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ---------- quickcheck (forked crate with `etna` feature) ----------

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_replace_all_utf8_safe(
    BytePatterns(patterns): BytePatterns,
    Utf8Haystack(haystack): Utf8Haystack,
    ShortRepl(replacement): ShortRepl,
) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_replace_all_utf8_safe(patterns, haystack, replacement) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_find_iter_prefilter_parity(
    PatternSet(patterns): PatternSet,
    MixHaystack(haystack): MixHaystack,
) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_find_iter_prefilter_parity(patterns, haystack) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let budget = cases_budget();
    let mut qc = QuickCheck::new()
        .tests(budget)
        .max_tests(budget.saturating_mul(2))
        .max_time(Duration::from_secs(86_400));
    let result = match property {
        "ReplaceAllUtf8Safe" => qc.quicktest(
            qc_replace_all_utf8_safe as fn(BytePatterns, Utf8Haystack, ShortRepl) -> TestResult,
        ),
        "FindIterPrefilterParity" => qc.quicktest(
            qc_find_iter_prefilter_parity as fn(PatternSet, MixHaystack) -> TestResult,
        ),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("quickcheck aborted: {err:?}")),
        ResultStatus::TimedOut => Err("quickcheck timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "quickcheck gave up after {} tests",
            result.n_tests_passed
        )),
    };
    (status, Metrics { inputs, elapsed_us })
}

// ---------- crabcheck ----------

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_replace_all_utf8_safe(
    (BytePatterns(patterns), Utf8Haystack(haystack), ShortRepl(replacement)): (
        BytePatterns,
        Utf8Haystack,
        ShortRepl,
    ),
) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_replace_all_utf8_safe(patterns, haystack, replacement) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_find_iter_prefilter_parity(
    (PatternSet(patterns), MixHaystack(haystack)): (PatternSet, MixHaystack),
) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_find_iter_prefilter_parity(patterns, haystack) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cc_config = crabcheck_qc::Config {
        tests: cases_budget(),
    };
    let result = match property {
        "ReplaceAllUtf8Safe" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_replace_all_utf8_safe)
        }
        "FindIterPrefilterParity" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_find_iter_prefilter_parity)
        }
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("crabcheck timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "crabcheck gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => {
            Err(format!("crabcheck aborted: {error}"))
        }
    };
    (status, Metrics { inputs, elapsed_us })
}

// ---------- hegel ----------

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(cases_budget())
        .suppress_health_check(HealthCheck::all())
}

fn hg_draw_byte(tc: &TestCase) -> u8 {
    let idx = tc.draw(
        hgen::integers::<usize>()
            .min_value(0)
            .max_value(BYTE_POOL.len() - 1),
    );
    BYTE_POOL[idx]
}

fn hg_draw_hay_char(tc: &TestCase) -> char {
    let idx = tc.draw(
        hgen::integers::<usize>()
            .min_value(0)
            .max_value(HAY_CHARS.len() - 1),
    );
    HAY_CHARS[idx]
}

fn hg_draw_repl_char(tc: &TestCase) -> char {
    let idx = tc.draw(
        hgen::integers::<usize>()
            .min_value(0)
            .max_value(REPL_CHARS.len() - 1),
    );
    REPL_CHARS[idx]
}

fn hg_draw_suffix_char(tc: &TestCase) -> char {
    let idx = tc.draw(
        hgen::integers::<usize>()
            .min_value(0)
            .max_value(PAT_SUFFIX_CHARS.len() - 1),
    );
    PAT_SUFFIX_CHARS[idx]
}

fn hg_draw_prefix(tc: &TestCase) -> &'static str {
    let idx = tc.draw(
        hgen::integers::<usize>()
            .min_value(0)
            .max_value(PAT_PREFIXES.len() - 1),
    );
    PAT_PREFIXES[idx]
}

fn hg_draw_byte_patterns(tc: &TestCase) -> Vec<Vec<u8>> {
    let n = tc.draw(hgen::integers::<usize>().min_value(1).max_value(4));
    (0..n)
        .map(|_| {
            let plen = tc.draw(hgen::integers::<usize>().min_value(1).max_value(4));
            (0..plen).map(|_| hg_draw_byte(tc)).collect()
        })
        .collect()
}

fn hg_draw_utf8_haystack(tc: &TestCase) -> String {
    let len = tc.draw(hgen::integers::<usize>().min_value(0).max_value(32));
    (0..len).map(|_| hg_draw_hay_char(tc)).collect()
}

fn hg_draw_short_repl(tc: &TestCase) -> String {
    let len = tc.draw(hgen::integers::<usize>().min_value(0).max_value(4));
    (0..len).map(|_| hg_draw_repl_char(tc)).collect()
}

fn hg_draw_pattern_set(tc: &TestCase) -> Vec<String> {
    let n = tc.draw(hgen::integers::<usize>().min_value(4).max_value(5));
    let mut patterns: Vec<String> = Vec::with_capacity(n);
    for _ in 0..n {
        let prefix = hg_draw_prefix(tc).to_string();
        let suf_len = tc.draw(hgen::integers::<usize>().min_value(0).max_value(3));
        let suffix: String = (0..suf_len).map(|_| hg_draw_suffix_char(tc)).collect();
        let pat = format!("{prefix}{suffix}");
        if !pat.is_empty() && !patterns.contains(&pat) {
            patterns.push(pat);
        }
    }
    if patterns.is_empty() {
        patterns.push("abcd".to_string());
    }
    let base = patterns[0].clone();
    let extra_len = tc.draw(hgen::integers::<usize>().min_value(1).max_value(3));
    let extra: String = (0..extra_len).map(|_| hg_draw_suffix_char(tc)).collect();
    let superstring = format!("{base}{extra}");
    if !patterns.contains(&superstring) {
        let insert_at = patterns.len().min(1);
        patterns.insert(insert_at, superstring);
    }
    patterns
}

fn hg_draw_mix_haystack(tc: &TestCase, patterns: &[String]) -> String {
    let n = tc.draw(hgen::integers::<usize>().min_value(1).max_value(6));
    let mut s = String::new();
    for _ in 0..n {
        let choice = tc.draw(hgen::integers::<u8>().min_value(0).max_value(2));
        match choice {
            0 => {
                if !patterns.is_empty() {
                    let pi = tc.draw(
                        hgen::integers::<usize>()
                            .min_value(0)
                            .max_value(patterns.len() - 1),
                    );
                    s.push_str(&patterns[pi]);
                }
            }
            1 => {
                let len = tc.draw(hgen::integers::<usize>().min_value(1).max_value(6));
                for _ in 0..len {
                    s.push(hg_draw_suffix_char(tc));
                }
            }
            _ => s.push(' '),
        }
    }
    s
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match property {
        "ReplaceAllUtf8Safe" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let patterns = hg_draw_byte_patterns(&tc);
                let haystack = hg_draw_utf8_haystack(&tc);
                let replacement = hg_draw_short_repl(&tc);
                let cex_p = patterns.clone();
                let cex_h = haystack.clone();
                let cex_r = replacement.clone();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_replace_all_utf8_safe(patterns, haystack, replacement)
                }));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        panic!("({:?} {:?} {:?})", cex_p, cex_h, cex_r)
                    }
                }
            })
            .settings(settings.clone())
            .run();
        }
        "FindIterPrefilterParity" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let patterns = hg_draw_pattern_set(&tc);
                let haystack = hg_draw_mix_haystack(&tc, &patterns);
                let cex_p = patterns.clone();
                let cex_h = haystack.clone();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_find_iter_prefilter_parity(patterns, haystack)
                }));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        panic!("({:?} {:?})", cex_p, cex_h)
                    }
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{}", property),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

// ---------- dispatch ----------

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (
            Err(format!("Unknown tool: {tool}")),
            Metrics::default(),
        ),
    }
}

fn json_str(s: &str) -> String {
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

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!("Properties: ReplaceAllUtf8Safe | FindIterPrefilterParity | All");
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic with non-string payload".to_string()
            };
            emit_json(tool, property, "aborted", Metrics::default(), None, Some(&msg));
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(e) => emit_json(tool, property, "failed", metrics, Some(&e), None),
    }
}
