# aho-corasick — ETNA Tasks

Total tasks: 8

ETNA tasks are **mutation/property/witness triplets**. Each row below is one runnable task. The `<PropertyKey>` token in the command column uses the PascalCase key recognised by `src/bin/etna.rs`; passing `All` runs every property for the named framework in a single invocation.

## Property keys

| Property | PropertyKey |
|----------|-------------|
| `property_replace_all_utf8_safe` | `ReplaceAllUtf8Safe` |
| `property_find_iter_prefilter_parity` | `FindIterPrefilterParity` |

## Task Index

| Task | Variant | Framework | Property | Witness | Command |
|------|---------|-----------|----------|---------|---------|
| 001 | `replace_all_utf8_boundary_e453f60_1` | proptest | `property_replace_all_utf8_safe` | `witness_replace_all_utf8_safe_case_split_codepoint` | `cargo run --release --bin etna -- proptest ReplaceAllUtf8Safe` |
| 002 | `replace_all_utf8_boundary_e453f60_1` | quickcheck | `property_replace_all_utf8_safe` | `witness_replace_all_utf8_safe_case_split_codepoint` | `cargo run --release --bin etna -- quickcheck ReplaceAllUtf8Safe` |
| 003 | `replace_all_utf8_boundary_e453f60_1` | crabcheck | `property_replace_all_utf8_safe` | `witness_replace_all_utf8_safe_case_split_codepoint` | `cargo run --release --bin etna -- crabcheck ReplaceAllUtf8Safe` |
| 004 | `replace_all_utf8_boundary_e453f60_1` | hegel | `property_replace_all_utf8_safe` | `witness_replace_all_utf8_safe_case_split_codepoint` | `cargo run --release --bin etna -- hegel ReplaceAllUtf8Safe` |
| 005 | `prefilter_pattern_id_sync_2df0983_1` | proptest | `property_find_iter_prefilter_parity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` | `cargo run --release --bin etna -- proptest FindIterPrefilterParity` |
| 006 | `prefilter_pattern_id_sync_2df0983_1` | quickcheck | `property_find_iter_prefilter_parity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` | `cargo run --release --bin etna -- quickcheck FindIterPrefilterParity` |
| 007 | `prefilter_pattern_id_sync_2df0983_1` | crabcheck | `property_find_iter_prefilter_parity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` | `cargo run --release --bin etna -- crabcheck FindIterPrefilterParity` |
| 008 | `prefilter_pattern_id_sync_2df0983_1` | hegel | `property_find_iter_prefilter_parity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` | `cargo run --release --bin etna -- hegel FindIterPrefilterParity` |

## Witness catalog

Each witness is a deterministic concrete test. Base build: passes. Variant-active build: fails. Witnesses live in `tests/etna_witnesses.rs`.

| Witness | Property | Detects | Input shape |
|---------|----------|---------|-------------|
| `witness_replace_all_utf8_safe_case_split_codepoint` | `property_replace_all_utf8_safe` | `replace_all_utf8_boundary_e453f60_1` | Pattern `[0xC3]` (lead byte of `é`), haystack `"éx"`, replacement `"Z"` — the single-byte match lands on a non-codepoint boundary |
| `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` | `property_find_iter_prefilter_parity` | `prefilter_pattern_id_sync_2df0983_1` | Patterns `["abcd", "abcdxy", "mnop", "wxyz"]`, haystack `"abcdxy abcd mnop wxyz"` — "abcdxy" is short-circuited under LeftmostFirst; the packed prefilter drops it and misassigns IDs for "mnop" and "wxyz" |
