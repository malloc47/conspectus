---
id: CSP-718
title: Guard the framework's independence from Conspectus
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-717
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: high
type: task
ordinal: 804000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150 lists independence rules and says each is enforced by a test, not by convention: the framework's manifest names no `conspectus` crate and its dependency set is an allowlist; no source, doc, or test file uses Conspectus vocabulary; the crate starts no processes and writes no files; no domain type leaks through its public API. Without these tests, a convenient `Msg` or harness color creeps back in the first time a control is awkward to make generic.

Scope: Add tests in `crates/tuikit` (or a workspace-level test) that: parse `crates/tuikit/Cargo.toml` and fail on a `conspectus` dependency or a dependency outside a documented allowlist; scan framework source, docs, and tests for a documented list of domain words (session, mux, harness, pin, worktree, PR, graph node, and the like) with a small, reviewed allowlist for exceptions; scan non-test source for process spawning and file writes (`std::process`, `Command`, `File::create`, `OpenOptions`, `fs::write`, `remove_file`), allowing read-only filesystem access for path completion; and fail when a scan visits fewer files than a stated minimum. Add a self-test that feeds each scan a deliberately leaking fixture and expects a failure.

TUI changes: none expected.

Tests and checks: Each guard fails on its deliberate-violation fixture and passes on the real tree. The documented allowlists are short and each entry has a reason.

Blockers: The workspace conversion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test fails when `crates/tuikit/Cargo.toml` depends on `conspectus` or on a crate outside the documented allowlist
- [ ] #2 A test fails when framework source, docs, or tests use a listed Conspectus domain word outside the reviewed allowlist
- [ ] #3 A test fails when framework non-test source spawns a process or writes a file
- [ ] #4 Each scan has a tripwire that fails if it visits fewer files than a stated minimum
- [ ] #5 A deliberate-violation fixture proves each guard can fail
- [ ] #6 The guard rules and how to request an allowlist entry are written in the framework's contribution guide
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
