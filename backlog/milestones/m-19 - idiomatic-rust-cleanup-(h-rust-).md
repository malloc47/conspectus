---
id: m-19
title: "Idiomatic Rust Cleanup (H-RUST-*)"
---

## Description

Source: a 2026-09-30 code review of the whole crate aimed at what an
experienced Rust reviewer would flag. Inputs were clippy's `pedantic`
and `nursery` groups (2,136 hits, most of them stylistic noise), greps
for panics, globals, `#[allow]`s, and history comments, `cargo doc`,
and hand reading of the model, discovery, resolver, server, and TUI
modules.

Goal: a vanilla codebase that is efficient, approachable, and not
trying to be fancy. Behavior-preserving, low-risk fixes landed during
the review (`CSP-554..562`). The rest are larger, need a decision,
or touch the public library surface, so they are queued as chunks
below (`CSP-563` onward), ordered by value and risk.

Scope bounds: no behavior changes beyond what a chunk names; every
chunk keeps `just check` green (which now includes `cargo doc` with
warnings as errors). Chunks that change the public library surface or
the serialized model note the ADR they need to touch.

Landed during the review:

- **CSP-554** Apply idiomatic clippy fixes and enforce them (`a4eaf2f`)
- **CSP-555** Use sets where maps held `()` or placeholder values (`5c3e2f7`)
- **CSP-556** Look up nodes without cloning their ids (`30f1f7d`)
- **CSP-557** Remove tombstone comments ("X moved to Y in wave N") and five copies of a current-epoch helper (`a381856`)
- **CSP-558** Stop deep-cloning `GraphSnapshot` to release a borrow in TUI pin/rename executors…
- **CSP-559** Remove two speculative traits: `MultiSelectItem` became `AsRef<str>` (`48ef2f4`)…
- **CSP-560** Fix all 44 rustdoc warnings and add a warnings-as-errors `cargo doc` step to `just check` and CI (`aaf4a05`)
- **CSP-561** Remove dead code hidden by `#[allow(dead_code)]`…
- **CSP-562** Recover poisoned locks in the daemon with `lock().unwrap_or_else(PoisonError::into_inner)` instead of ten hand-written…

Queued chunks:

- **CSP-563** Replace the process-global discovery caches
- **CSP-564** Give the library typed errors
- **CSP-565** Narrow the public surface
- **CSP-566** Strip backlog IDs from code comments
- **CSP-567** Use typed kinds instead of strings
- **CSP-568** Split the 3,000-line TUI modules
- **CSP-569** Retire `SessionViewerAction`
- **CSP-570** Rename `GraphDb`
- **CSP-571** Smaller follow-ups, as files are touched
- **CSP-572** Extend lint enforcement after the chunks land
- **CSP-573** Decide how Codex-log evidence ranks in mux resolution

- **CSP-579** Don't abort discovery on one unreadable scan-root child
