# aho-corasick — Injected Bugs

Total mutations: 2

## Bug Index

| # | Name | Variant | File | Injection | Fix Commit |
|---|------|---------|------|-----------|------------|
| 1 | `replace_all_utf8_boundary` | `replace_all_utf8_boundary_e453f60_1` | `patches/replace_all_utf8_boundary_e453f60_1.patch` | `patch` | `e453f6062651f338ca3a7fb217f7590a2f89f9ab` |
| 2 | `prefilter_pattern_id_sync` | `prefilter_pattern_id_sync_2df0983_1` | `patches/prefilter_pattern_id_sync_2df0983_1.patch` | `patch` | `2df0983e7ef76bf50e6213fee35cda87af191292` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `replace_all_utf8_boundary_e453f60_1` | `property_replace_all_utf8_safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |
| `prefilter_pattern_id_sync_2df0983_1` | `property_find_iter_prefilter_parity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `property_replace_all_utf8_safe` | OK | OK | OK | OK |
| `property_find_iter_prefilter_parity` | OK | OK | OK | OK |

## Bug Details

### 1. replace_all_utf8_boundary (e453f60_1)
- **Variant**: `replace_all_utf8_boundary_e453f60_1`
- **Location**: `src/automaton.rs`, `try_replace_all_with`
- **Property**: `property_replace_all_utf8_safe`
- **Witness**: `witness_replace_all_utf8_safe_case_split_codepoint`
- **Fix commit**: `e453f6062651f338ca3a7fb217f7590a2f89f9ab` — "fuzz: fix a bug caught by the fuzzer"
- **Invariant violated**: `AhoCorasick::replace_all(haystack, patterns, &[replacement; N])` never panics when the patterns are given as raw bytes and the haystack is a valid `&str`, even when a byte-level pattern match would split a multi-byte UTF-8 scalar. The function must silently skip matches that land on non-codepoint boundaries.
- **How the mutation triggers**: The fix added an `if !haystack.is_char_boundary(m.start()) || !haystack.is_char_boundary(m.end()) { continue; }` guard in `try_replace_all_with`. Reverting the guard means `&haystack[..m.start()]` is evaluated on byte offsets that are not char boundaries, and Rust's `&str` indexing panics with `byte index X is not a char boundary`. The property's witness drives this with the single-byte pattern `0xC3` (the lead byte of `é`) against the haystack `"éx"`: the match at `start=0, end=1` lands between the two bytes of `é`, and the downstream `&haystack[..1]` slice panics.

### 2. prefilter_pattern_id_sync (2df0983_1)
- **Variant**: `prefilter_pattern_id_sync_2df0983_1`
- **Location**: `src/nfa/noncontiguous.rs`, `Compiler::build_trie` pattern-compile loop
- **Property**: `property_find_iter_prefilter_parity`
- **Witness**: `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision`
- **Fix commit**: `2df0983e7ef76bf50e6213fee35cda87af191292` — "prefilter: fix a prefilter bug"
- **Invariant violated**: `AhoCorasick::find_iter` returns the same `(PatternID, start, end)` sequence regardless of whether the prefilter is enabled. Since the prefilter is a pure runtime acceleration, it must not change the set of matches — especially not the reported pattern IDs.
- **How the mutation triggers**: The packed prefilter assigns pattern IDs in the order `self.prefilter.add(pat)` is called, independent of the Aho-Corasick NFA's IDs. In leftmost-first mode, a pattern whose prefix coincides with an earlier accepting state is short-circuited (`continue 'PATTERNS`) during compilation. The fix commit registers each pattern with the prefilter *before* the pattern walk, so every NFA pattern ID has a corresponding prefilter ID even when the walk short-circuits. The mutation moves `self.prefilter.add(pat)` to *after* the pattern walk: short-circuited patterns never reach it, and every later pattern ends up one (or more) IDs too low in the prefilter. When the packed prefilter reports `Candidate::Match(pattern_id=k)` on a haystack hit, the automaton looks up match state `k` and reports the wrong `PatternID` — diverging from the prefilter-disabled run. The witness uses `["abcd", "abcdxy", "mnop", "wxyz"]` and haystack `"abcdxy abcd mnop wxyz"`: `"abcdxy"` is short-circuited, so `"mnop"` and `"wxyz"` land at prefilter IDs 1 and 2 while their NFA IDs are 2 and 3 — visible as diverging `find_iter` output across the two build configurations.
