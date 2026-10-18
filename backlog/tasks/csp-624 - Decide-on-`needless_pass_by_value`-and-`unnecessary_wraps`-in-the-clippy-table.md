---
id: CSP-624
title: Decide on `needless_pass_by_value` and `unnecessary_wraps` in the clippy table
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 01:10'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 682000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-466 built the curated `[lints.clippy]` table in `Cargo.toml` and recorded why `match_same_arms`, the doc lints, and the cast lints stay off. Two lints the audit flagged got neither a deny nor a recorded allow. On 2026-10-16 `needless_pass_by_value` reports 66 sites, up from 60 at audit time, most in `tui/runtime/worktree_exec.rs` (8), `tui/runtime/pin_store.rs` (4), `server/mod.rs` (4), and `cli/mod.rs` (4); each one makes callers clone or give up a value the callee only reads. `unnecessary_wraps` reports about a dozen functions that always return `Ok` or `Some`, several in `tui/runtime.rs`. Without a recorded decision the counts keep growing. Audit finding C1.

- Scope: for each lint, deny it and fix the fallout, or allow it with a one-line rationale in `Cargo.toml` as `match_same_arms` has. A per-site `#[allow]` with a reason is fine where taking ownership is deliberate, such as thread spawns.
- Tests: `just check` with the updated table.
- Blockers: none. CSP-619 removes some `unnecessary_wraps` hits in the fixture-mode overlay copies, so doing it first saves churn.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Cargo.toml` records a deny, or an allow with a rationale, for both lints
- [x] #2 `just check` passes with the updated table
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-measure both lints (83 hits on 2026-10-17: ~70 pass-by-value, 12 wrap).
2. `needless_pass_by_value`: allow with a rationale in `Cargo.toml`. The hits are named-argument input structs, small enums, thread entry points, and values moved out of effect enums; references would make those APIs worse without saving copies.
3. `unnecessary_wraps`: deny. Fix the nine always-`Ok`/`Some` functions and their callers; allow at the three clap subcommand `run` methods, with a reason, because they share the dispatcher's `run(self) -> Result<()>` shape.
4. `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `needless_pass_by_value = "allow"` with a four-line rationale beside `match_same_arms` in `[lints.clippy]`.
- `unnecessary_wraps = "deny"`. Fixed: `cli::candidate_store_paths` and `candidate_pin_store_paths` (now return `Vec`), Pi `discover_state` (returns `GraphFragment`; the adapter wraps it), `tree_current_unix_epoch` (returns `i64`; callers wrap), the three pin `*_cursor_line` helpers (return `usize`), and the static-mode `refresh_from_snapshot` / `set_static_data` (no `Result`). Allowed with `reason`: the `run` methods of the `alias`, `columns`, and `declared` subcommands.
- Validation: `cargo clippy -- -W clippy::unnecessary_wraps` reports 0 hits; `just check` (clippy with `-D warnings`) green, 2411 tests.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Both lints now have a recorded decision in `Cargo.toml`. `unnecessary_wraps` is denied: nine functions that always returned `Ok`/`Some` return plain values, and the three subcommand `run` methods that keep the dispatcher's `Result` shape carry a reasoned `#[allow]`. `needless_pass_by_value` is allowed with a rationale: its hits are deliberate by-value idioms (input structs, small enums, thread hand-offs). Verified with `just check` (2411 tests) and zero remaining `unnecessary_wraps` hits.
<!-- SECTION:FINAL_SUMMARY:END -->
