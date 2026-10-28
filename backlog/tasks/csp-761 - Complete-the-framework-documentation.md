---
id: CSP-761
title: Complete the framework documentation
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-738
  - CSP-737
  - CSP-740
  - CSP-743
  - CSP-744
  - CSP-748
  - CSP-747
  - CSP-749
  - CSP-753
  - CSP-756
  - CSP-758
  - CSP-760
  - CSP-732
  - CSP-720
documentation:
  - docs/plans/tui-framework/gap-analysis.md
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: medium
type: docs
ordinal: 847000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The skeleton from the documentation story gives every control a page template, and each extraction story fills in its own page. This story checks that the whole is complete and usable on its own: someone in another repository should be able to learn the framework, add a control, and find the house rules without reading Conspectus.

Scope: Verify and fill gaps: a reference page for every public control and foundation module, a usage guide for building an app on the framework (the `M` and `Cx` parameters, hosting the modal stack, registering theme roles, writing key tables, using the test kit), the house rules as a reference, and a roadmap built from `docs/plans/tui-framework/gap-analysis.md` trimmed to the framework's own terms. Add a doc-sync test, in the style of `tests/tui_columns_doc.rs`, that fails when a public control has no reference page or a page names a control that does not exist.

TUI changes: none.

Tests and checks: `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passes. The doc-sync test fails on a scratch change that adds a public control without a page.

Blockers: Every control story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every public control and foundation module has a reference page, and a doc-sync test fails when one is missing or stale
- [ ] #2 A usage guide explains hosting the modal stack, registering theme roles, writing key tables, and using the test kit
- [ ] #3 The house rules are published as a reference and the roadmap lists the gap-analysis tiers
- [ ] #4 The framework docs make no reference to Conspectus-only files, types, or commands
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
