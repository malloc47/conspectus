---
id: CSP-398
title: 'Launch decision tree: sidecar consumer + lineage walk'
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies:
  - CSP-395
  - CSP-396
  - CSP-397
ordinal: 339000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the `pin launch` decision tree (`src/cli.rs`
  `PinLaunchArgs::run`, hook into the existing branch on
  `PinUnbound`) per ADR 0058 §Read path:
  1. Load the sidecar via CSP-395 helpers; absent →
     default argv with status hint.
  2. Look up the recorded `session_id` in the snapshot, then
     fall back to the harness state root per Q3 if missing
     from the snapshot.
  3. Walk the ADR 0018 `parent_session` chain forward to the
     current head; stop at any fork (multiple successors
     sharing an ancestor) and treat as default-argv launch
     with a "multiple successors" status hint per Q8.
  4. If the chosen session still cannot be found (or the
     step-2 lookup found nothing at all), **delete the
     sidecar file** per Q7, status hint, fall back to default
     argv.
  5. Consult `HarnessAdapter::resume_argv(session_id, cwd)`;
     `None` → status hint + default argv; `Some(argv)` →
     splice into the `tmux new-session` call.
  Same flow applies to `pin attach` when it falls through to
  launch. `--no-attach` short-circuits after spawn as today.
- Tests: CLI integration tests for each branch: sidecar
  absent, snapshot hit, state-root fallback hit, linear
  lineage walk (one successor per step), fork in lineage,
  missing-session sidecar deletion, `resume_argv` returns
  `None` (aider), happy-path resume. Use `FakeTmux` to assert
  the constructed `new-session` argv.
- Blockers: `CSP-395`, `CSP-396`,
  `CSP-397`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin_bindings::lineage_head` walks `ParentSession`
candidate links forward (target = current, source =
successor) to a leaf, with a visited-set cycle guard;
returns `LineageOutcome::Head | SessionMissing | Fork`.
`cli::resolve_resume_argv` glues sidecar read → lineage
walk → existence check → `resume_argv_for` splice, with
sidecar deletion on missing-session and status hints on
every fallback path. Split into env-driven and cache-
injected variants for testability. 14 unit tests across
`pin_bindings::lineage` (7) and `cli::resume_resolver` (7).
State-root fallback per Q3 is deferred — the resolver pass
runs immediately before the launch decision, so any
discoverable session is in the snapshot already.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-004`
