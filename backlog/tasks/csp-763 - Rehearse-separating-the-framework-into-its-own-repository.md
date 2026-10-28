---
id: CSP-763
title: Rehearse separating the framework into its own repository
status: To Do
assignee: []
created_date: '2026-10-27 20:28'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
  - CSP-718
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: high
type: task
ordinal: 849000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150's goal is that the framework can move to its own repository at any time. That claim is only true if someone has done it: an unproven separation tends to fail on a workspace-inherited field, a relative path, or a test fixture that quietly reads a Conspectus file. A rehearsal that runs in CI keeps the property true afterward.

Scope: Write a script (for example `scripts/export-tuikit.sh`) that produces a standalone repository from `crates/tuikit`, keeping its history (`git subtree split`), and adds the root files a repository needs: a toolchain file, license, `.gitignore`, a CI workflow, a `justfile`, and a minimal dev-shell flake. It rewrites workspace-inherited manifest fields so the crate resolves alone. In a scratch directory, formatting, clippy with warnings denied, all tests with all features, and docs with warnings denied pass with no Conspectus checkout present. Run the rehearsal in CI (or as a `just` recipe that CI calls) on changes under `crates/tuikit`, and document the real move step by step in the framework docs.

TUI changes: none.

Tests and checks: Run the script on a clean clone and confirm the four checks pass from the scratch export. Introduce a deliberate leak (a relative path to `../../src`) and confirm the rehearsal fails.

Blockers: Complete documentation and the independence guard.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A script exports `crates/tuikit` with its history and the root files into a scratch directory
- [ ] #2 In the export, formatting, clippy (warnings denied), all-feature tests, and docs (warnings denied) pass with no Conspectus files present
- [ ] #3 CI runs the rehearsal on changes under `crates/tuikit`
- [ ] #4 A deliberate leak makes the rehearsal fail
- [ ] #5 The framework docs describe the real separation procedure
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
