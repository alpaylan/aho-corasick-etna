# aho-corasick — ETNA Tasks

Total tasks: 8

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `prefilter_pattern_id_sync_2df0983_1` | proptest | `FindIterPrefilterParity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |
| 002 | `prefilter_pattern_id_sync_2df0983_1` | quickcheck | `FindIterPrefilterParity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |
| 003 | `prefilter_pattern_id_sync_2df0983_1` | crabcheck | `FindIterPrefilterParity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |
| 004 | `prefilter_pattern_id_sync_2df0983_1` | hegel | `FindIterPrefilterParity` | `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` |
| 005 | `replace_all_utf8_boundary_e453f60_1` | proptest | `ReplaceAllUtf8Safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |
| 006 | `replace_all_utf8_boundary_e453f60_1` | quickcheck | `ReplaceAllUtf8Safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |
| 007 | `replace_all_utf8_boundary_e453f60_1` | crabcheck | `ReplaceAllUtf8Safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |
| 008 | `replace_all_utf8_boundary_e453f60_1` | hegel | `ReplaceAllUtf8Safe` | `witness_replace_all_utf8_safe_case_split_codepoint` |

## Witness Catalog

- `witness_find_iter_prefilter_parity_case_leftmost_prefix_collision` — base passes, variant fails
- `witness_replace_all_utf8_safe_case_split_codepoint` — base passes, variant fails
