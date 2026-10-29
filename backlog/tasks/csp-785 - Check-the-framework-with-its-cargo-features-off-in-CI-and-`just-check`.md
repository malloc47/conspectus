---
id: CSP-785
title: Check the framework with its cargo features off in CI and `just check`
status: To Do
assignee: []
created_date: '2026-10-29 22:32'
labels:
  - tui-framework
milestone: m-25
dependencies: []
ordinal: 871000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The framework crate has features a host turns on (`richtext`, which pulls in the rich-text crates, and `testkit`), and ADR 0152 and ADR 0181 promise that `tuikit` builds and passes its tests with them off, so a program that shows no rich content pays for none of the parsing crates. CI and `just check` run only `--all-features`, and the workspace's feature unification turns `richtext` on whenever the host is built, so a change that makes code outside `tuikit::richtext` need the feature (or leaves an item only that module uses ungated, which warns without it) passes every check today and is found only by hand. CSP-749 ran the feature-off build manually (`cargo clippy -p tuikit --all-targets -- -D warnings`, `cargo test -p tuikit --no-default-features`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CI and `just check` build, lint (warnings denied), and test `tuikit` with its features off, and with `richtext` alone
- [ ] #2 Code outside `tuikit::richtext` that names one of its items, or an item only that module uses left ungated, makes the check fail
- [ ] #3 The framework contribution guide and `docs/plans/tui-framework/workspace-impact.md` name the check
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
