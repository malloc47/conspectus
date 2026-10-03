---
id: CSP-554
title: Apply idiomatic clippy fixes and enforce them (`a4eaf2f`)
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 563000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`let ... else` and `if let` instead of single-arm `match`, flattened or-patterns, method references instead of forwarding closures, no `collect()` just to count, `find_map`, `into_iter()` instead of `drain(..)` on owned Vecs, `Path::extension` instead of `ends_with(".jsonl")`, and similar. Twenty lints were added to `[lints.clippy]` as `deny`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RUST-001`
