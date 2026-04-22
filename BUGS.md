# aho-corasick — Injected Bugs

ETNA workload for the Rust `aho-corasick` crate. Variants are patch-based:
each re-introduces one historical bug-fix on top of the master base commit
and pairs it with a framework-neutral property, four PBT adapters, and a
deterministic witness test.

Total mutations: 2

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `prefilter_pattern_id_sync_2df0983_1` | `prefilter_pattern_id_sync` | `src/nfa/noncontiguous.rs` | `patch` | `2df0983e7ef76bf50e6213fee35cda87af191292` |
| 2 | `replace_all_utf8_boundary_e453f60_1` | `replace_all_utf8_boundary` | `src/automaton.rs` | `patch` | `e453f6062651f338ca3a7fb217f7590a2f89f9ab` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `prefilter_pattern_id_sync_2df0983_1` | `FindIterPrefilterParity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |
| `replace_all_utf8_boundary_e453f60_1` | `ReplaceAllUtf8Safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `FindIterPrefilterParity` | ✓ | ✓ | ✓ | ✓ |
| `ReplaceAllUtf8Safe` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. prefilter_pattern_id_sync

- **Variant**: `prefilter_pattern_id_sync_2df0983_1`
- **Location**: `src/nfa/noncontiguous.rs` (inside `Compiler::build_trie`)
- **Property**: `FindIterPrefilterParity`
- **Witness(es)**:
  - `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision`
- **Source**: prefilter: fix a prefilter bug
  > A pattern-ID desync between the Aho-Corasick NFA and the packed prefilter. In leftmost-first mode, a pattern whose prefix is already accepted short-circuits NFA compilation, so the prefilter's pattern-ID counter fell behind and later patterns ended up with the wrong IDs — producing mismatched `find_iter` output depending on whether the prefilter was enabled.
- **Fix commit**: `2df0983e7ef76bf50e6213fee35cda87af191292` — prefilter: fix a prefilter bug
- **Invariant violated**: `AhoCorasick::find_iter` returns the same `(PatternID, start, end)` sequence regardless of whether the prefilter is enabled. Since the prefilter is a pure runtime acceleration, it must not change the set of matches — especially not the reported pattern IDs.
- **How the mutation triggers**: The packed prefilter assigns pattern IDs in the order `self.prefilter.add(pat)` is called, independent of the Aho-Corasick NFA's IDs. In leftmost-first mode, a pattern whose prefix coincides with an earlier accepting state is short-circuited (`continue 'PATTERNS`) during compilation. The fix commit registers each pattern with the prefilter *before* the pattern walk, so every NFA pattern ID has a corresponding prefilter ID even when the walk short-circuits. The mutation moves `self.prefilter.add(pat)` to *after* the pattern walk: short-circuited patterns never reach it, and every later pattern ends up one (or more) IDs too low in the prefilter. When the packed prefilter reports `Candidate::Match(pattern_id=k)` on a haystack hit, the automaton looks up match state `k` and reports the wrong `PatternID` — diverging from the prefilter-disabled run. The witness uses `["abcd", "abcdxy", "mnop", "wxyz"]` and haystack `"abcdxy abcd mnop wxyz"`: `"abcdxy"` is short-circuited, so `"mnop"` and `"wxyz"` land at prefilter IDs 1 and 2 while their NFA IDs are 2 and 3 — visible as diverging `find_iter` output across the two build configurations.

### 2. replace_all_utf8_boundary

- **Variant**: `replace_all_utf8_boundary_e453f60_1`
- **Location**: `src/automaton.rs` (inside `try_replace_all_with`)
- **Property**: `ReplaceAllUtf8Safe`
- **Witness(es)**:
  - `witness_replace_all_utf8_safe_case_split_codepoint`
- **Source**: fuzz: fix a bug caught by the fuzzer
  > A fuzzer-found panic in `AhoCorasick::replace_all` when a byte-level pattern match lands on a non-codepoint boundary inside a `&str` haystack. The replace loop was indexing the haystack with the raw match range before verifying the range sits on char boundaries.
- **Fix commit**: `e453f6062651f338ca3a7fb217f7590a2f89f9ab` — fuzz: fix a bug caught by the fuzzer
- **Invariant violated**: `AhoCorasick::replace_all(haystack, patterns, &[replacement; N])` never panics when the patterns are given as raw bytes and the haystack is a valid `&str`, even when a byte-level pattern match would split a multi-byte UTF-8 scalar. The function must silently skip matches that land on non-codepoint boundaries.
- **How the mutation triggers**: The fix added an `if !haystack.is_char_boundary(m.start()) || !haystack.is_char_boundary(m.end()) { continue; }` guard in `try_replace_all_with`. Reverting the guard means `&haystack[..m.start()]` is evaluated on byte offsets that are not char boundaries, and Rust's `&str` indexing panics with `byte index X is not a char boundary`. The witness drives this with the single-byte pattern `0xC3` (the lead byte of `é`) against the haystack `"éx"`: the match at `start=0, end=1` lands between the two bytes of `é`, and the downstream `&haystack[..1]` slice panics.
