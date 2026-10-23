---
id: CSP-694
title: Represent entity state and age uniformly across the left-panel views
status: To Do
assignee: []
created_date: '2026-10-23 20:15'
labels:
  - tui
milestone: m-23
dependencies: []
priority: high
type: feature
ordinal: 764000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A column-by-column audit of the left panel found the same state drawn as a glyph in one view and a word in another, one glyph meaning three things, and age meaning different times in the same column. The operator decided the rules: state is a glyph in views of one primary entity (muxes, sessions, PRs) and a word only in mixed views (Pinned, History); every represented state has a glyph for both of its values; a stopped mux reads stopped and a removed worktree or repo reads removed everywhere; a chain-link style glyph marks attached or linked state; age is the most expected time axis for the view and the same for the same entity everywhere.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
