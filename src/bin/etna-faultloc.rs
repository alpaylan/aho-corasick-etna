// Crabcheck fault-localization runner for aho-corasick.
//
// Mirrors the BST/RBT/STLC `faultloc` binary: drives each property via
// `crabcheck::profiling::quickcheck`, which instruments every iteration
// with LLVM coverage, and on first failure takes 500 mutation snapshots
// plus `coverage/indices.json` for downstream SBFL analysis.
//
// Self-contained on purpose — wrapper types and their Arbitrary impls are
// duplicated from `src/bin/etna.rs` so the working Etna runner stays
// untouched. Mutate impls are new (BST-style structural perturbations).

use std::fmt;

use aho_corasick::etna::{
    property_find_iter_prefilter_parity, property_replace_all_utf8_safe, PropertyResult,
};
use crabcheck::profiling::{quickcheck, quickcheck_with_shrink};
use crabcheck::quickcheck::{Arbitrary, Mutate};
use rand::Rng;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for Utf8Haystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for ShortRepl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for PatternSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for MixHaystack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

// ---------- Generator pools (duplicated from src/bin/etna.rs) ----------

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

// Upper bounds. Match the discard guards in src/etna.rs so mutations
// don't immediately Discard. See etna.rs:42-52, 102-112.
const MAX_PATTERNS: usize = 4;
const MAX_PATTERN_LEN: usize = 4;
const MAX_HAYSTACK_CHARS: usize = 32;
const MAX_REPL_CHARS: usize = 4;
const MAX_PATTERN_SET: usize = 6;
const MAX_PATTERN_SET_STR_LEN: usize = 7;

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
            },
            1 => {
                let len = rng.random_range(1usize..=6);
                for _ in 0..len {
                    s.push(PAT_SUFFIX_CHARS[rng.random_range(0..PAT_SUFFIX_CHARS.len())]);
                }
            },
            _ => s.push(' '),
        }
    }
    s
}

// ---------- Arbitrary impls (crabcheck) ----------

impl<R: Rng> Arbitrary<R> for BytePatterns {
    fn generate(rng: &mut R, _n: usize) -> Self { BytePatterns(random_byte_patterns(rng)) }
}
impl<R: Rng> Arbitrary<R> for Utf8Haystack {
    fn generate(rng: &mut R, _n: usize) -> Self { Utf8Haystack(random_utf8_haystack(rng)) }
}
impl<R: Rng> Arbitrary<R> for ShortRepl {
    fn generate(rng: &mut R, _n: usize) -> Self { ShortRepl(random_short_repl(rng)) }
}
impl<R: Rng> Arbitrary<R> for PatternSet {
    fn generate(rng: &mut R, _n: usize) -> Self { PatternSet(random_pattern_set(rng)) }
}
impl<R: Rng> Arbitrary<R> for MixHaystack {
    fn generate(rng: &mut R, n: usize) -> Self {
        let ps = PatternSet::generate(rng, n).0;
        MixHaystack(random_mix_haystack(rng, &ps))
    }
}

// ---------- Mutate impls (BST-style single-point perturbation) ----------
//
// Each mutate preserves structure (pattern count, pattern lengths, string
// length) and changes exactly one byte or char. The analogue of BST's
// `mut_tree` keeping the tree shape intact while swapping one
// key/value/subtree. This keeps mutants close to the failing seed so
// the fault-localization signal stays strong; add/remove ops tended to
// escape the bug's precondition (especially the superstring relation
// in PatternSet that triggers the prefilter short-circuit).

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
        // Pick a pattern and flip one character in it. The short-circuit
        // bug depends on one pattern being a prefix of another — keeping
        // count and lengths fixed preserves that relation whenever it
        // exists in the seed.
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
        // Swap in a character from the pattern-suffix / space pool; the
        // mix remains a valid haystack of the same length.
        let pool: &[char] = &[
            'a', 'b', 'c', 'x', 'y', 'z', '0', '1', ' ',
        ];
        chars[i] = pool[rng.random_range(0..pool.len())];
        MixHaystack(chars.into_iter().collect())
    }
}

// ---------- PropertyResult → Option<bool> ----------

fn to_opt(r: PropertyResult) -> Option<bool> {
    match r {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

// ---------- Shrinkers ----------
//
// Classical QuickCheck-style shrinkers. Each returns a set of strictly
// smaller candidates; the crabcheck profiling loop accepts the first
// candidate that still fails the property and repeats until local
// minimum. For FindIterPrefilterParity specifically, shrinking typically
// reduces the failing seed from ~6 patterns / ~20-char haystack down to
// the 2-pattern superstring pair + the minimum haystack that matches
// both — which is the exact shape the bug needs to trigger.

fn shrink_find_iter(input: &(PatternSet, MixHaystack)) -> Vec<(PatternSet, MixHaystack)> {
    let (ps, mh) = input;
    let mut out = Vec::new();

    // 1. Remove one pattern (keep >= 2 so the superstring relation can survive).
    if ps.0.len() > 2 {
        for i in 0..ps.0.len() {
            let mut v = ps.0.clone();
            v.remove(i);
            out.push((PatternSet(v), mh.clone()));
        }
    }

    // 2. Remove one char from the haystack.
    let hchars: Vec<char> = mh.0.chars().collect();
    if hchars.len() > 1 {
        for i in 0..hchars.len() {
            let mut cs = hchars.clone();
            cs.remove(i);
            out.push((ps.clone(), MixHaystack(cs.into_iter().collect())));
        }
    }

    // 3. Drop the last char of one pattern (keep each pattern >= 1 char).
    for i in 0..ps.0.len() {
        let pchars: Vec<char> = ps.0[i].chars().collect();
        if pchars.len() > 1 {
            let mut v = ps.0.clone();
            v[i] = pchars[..pchars.len() - 1].iter().collect();
            if !v[i].is_empty() {
                out.push((PatternSet(v), mh.clone()));
            }
        }
    }

    out
}

// ---------- Dispatcher ----------

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property> [tests]", args[0]);
        eprintln!("  tool:     crabcheck");
        eprintln!("  property: ReplaceAllUtf8Safe | FindIterPrefilterParity");
        return;
    }
    let tool = args[1].as_str();
    let property = args[2].as_str();

    let result = match (tool, property) {
        ("crabcheck", "ReplaceAllUtf8Safe") => {
            quickcheck(|(BytePatterns(p), Utf8Haystack(h), ShortRepl(r))| {
                to_opt(property_replace_all_utf8_safe(p, h, r))
            })
        },
        ("crabcheck", "FindIterPrefilterParity") => {
            quickcheck_with_shrink(
                |(PatternSet(p), MixHaystack(h))| {
                    to_opt(property_find_iter_prefilter_parity(p, h))
                },
                shrink_find_iter,
            )
        },
        ("crabcheck", "FindIterPrefilterParityNoShrink") => {
            quickcheck(|(PatternSet(p), MixHaystack(h))| {
                to_opt(property_find_iter_prefilter_parity(p, h))
            })
        },
        _ => panic!("Unknown tool or property: {tool} {property}"),
    };

    println!("Result: {:?}", result);
}
