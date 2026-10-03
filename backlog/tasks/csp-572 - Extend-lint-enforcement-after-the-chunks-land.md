---
id: CSP-572
title: Extend lint enforcement after the chunks land
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 581000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Plan: once `CSP-566` and `CSP-571` are done, consider
  enforcing `items_after_statements`, `needless_pass_by_value` for
  non-message functions, and `rustdoc::private_intra_doc_links` in
  `[lints]`, so the cleanup holds.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Clippy now also denies `items_after_statements` (with
`CSP-571`), `if_not_else` (two sites fixed), and `implicit_clone`,
`manual_assert`, and `redundant_else` (no hits). `[lints.rustdoc]`
denies broken and private intra-doc links, so `cargo doc` enforces
them even without `RUSTDOCFLAGS`. Surveyed and left allowed:
`needless_pass_by_value` (72 hits, mostly handlers that take `Msg`
by design), `doc_markdown` (128), `similar_names` (15),
`too_many_lines` (28), `format_push_string` (21, see
`CSP-571`), `unnecessary_wraps` (signatures shared on purpose),
`needless_continue` (`=> continue` arms read clearly),
`trivially_copy_pass_by_ref` and `ref_option` (serde's `with` and
`skip_serializing_if` require the reference; the one real case,
`link_freshness_tag`, was fixed), and `unused_self` (test-scenario
helpers).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-019`
