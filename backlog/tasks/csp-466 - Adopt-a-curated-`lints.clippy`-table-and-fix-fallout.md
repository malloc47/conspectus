---
id: CSP-466
title: 'Adopt a curated `[lints.clippy]` table and fix fallout'
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies:
  - CSP-462
  - CSP-463
ordinal: 97000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Full stream landed 2026-07-04** across 5 waves:
  * Wave 1 (`2c44d15`): `[lints.clippy]` table shape +
    `redundant_clone = "deny"` (63 warnings auto-fixed).
  * Wave 2 (`04c0aff`): `match_wildcard_for_single_variants
    = "deny"` (29 warnings; every wildcard over
    `LinkEndpoint` given an explicit variant so a
    hypothetical third variant surfaces at compile time
    — synergistic with H-EXT-* compile-time safety).
  * Wave 3 (`2f24fa8`): `uninlined_format_args = "deny"`
    (12 warnings auto-fixed).
  * Wave 4 (`7caa2ec`): `map_unwrap_or = "deny"` (69
    warnings: `.map(f).unwrap_or(a)` → `map_or(a, f)`;
    `.map(f).unwrap_or_else(g)` → `map_or_else(g, f)`;
    `.map(f).unwrap_or(false)` → `is_some_and(f)`).
  * Wave 5 (`4db88f9`): `match_same_arms` documented as
    intentional `allow` — the 36 current warnings sit in
    documentation-shaped match tables where merging arms
    into `|` patterns would sacrifice per-row readability.
    Rationale committed to `Cargo.toml` comment.
- Skipped per audit (durable): doc lints
  (Conspectus doc culture is ADRs + module `//!` headers,
  not per-fn docs) and cast-truncation lints in TUI
  layout modules (allowed with per-module `#![allow]`
  where the truncation is intentional).
- Blockers: `CSP-462`/`CSP-463` landed.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-005`
