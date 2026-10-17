---
id: CSP-624
title: Decide on `needless_pass_by_value` and `unnecessary_wraps` in the clippy table
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 `Cargo.toml` records a deny, or an allow with a rationale, for both lints
- [ ] #2 `just check` passes with the updated table
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
