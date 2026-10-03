---
id: CSP-457
title: Launch argv editor with resolved command preview
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-454
ordinal: 324000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: make launch customization usable for sandbox/wrapper
  workflows. The create form should let the operator edit argv as a
  structured command, show the effective command that will run
  after harness defaults or per-pin overrides are applied, and make
  it obvious when the default adapter command is being used versus
  a pin-specific override. Do not add lifecycle hooks here; richer
  before/after hooks remain `CSP-383`.
- Tests: reducer/widget tests for default command preview,
  override editing, shell-like display escaping without shell-based
  execution, clearing back to default, and validation errors for an
  empty argv override.
- Blockers: `CSP-454`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The create form now previews the effective command below
the `launch argv` editor. A blank editor means "use the selected
harness adapter default" and renders as `default: <command>`;
nonblank input is parsed as a small shell-style argv override and
renders as `override: <command>`. Clearing the field returns to
the adapter default. Unknown/custom harnesses without an explicit
launch argv are rejected before commit so `pin launch` will not
later fail with no command. Preview display quotes whitespace
arguments without shell execution.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-006`
