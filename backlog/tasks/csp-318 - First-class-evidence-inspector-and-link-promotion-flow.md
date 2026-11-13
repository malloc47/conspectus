---
id: CSP-318
title: First-class evidence inspector and link-promotion flow
status: To Do
assignee: []
created_date: '2026-06-01 03:32'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-316
ordinal: 422000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the v1 `o opens evidence` placeholder on
  unresolved-evidence rows (per
  `docs/tui-detail-mockup.md`) with a focused inspector that
  renders all `UnresolvedEndpoint` metadata for the candidate
  (`harness_key`, `native_id`, `state_scope`, `path`, free-form
  `metadata` fields) and supports promoting that evidence to a
  durable declared link from inside the TUI. The same inspector
  should let operators convert a discovered/resolved candidate
  into a declared link without leaving the detail explorer.
  Writes route through the existing declared-link CRUD path
  (server socket or direct SQLite writer per ADR 0038); the
  inspector does not bypass the manual-link command surface in
  `docs/design.md`.
- Tests: reducer/keymap tests for opening the inspector,
  cancelling, and promoting evidence; declared-link store
  integration tests that confirm the write lands in the
  appropriate local-or-global store per `docs/design.md`'s
  persistence rules; snapshot coverage for the inspector with
  sparse vs richly-populated unresolved endpoints.
- Blockers: `CSP-316`, declared-link CRUD landing in the TUI
  surface (currently CLI-only).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-032`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the right pane is the framework explorer over the host's source (ADR 0188), and its rows are a `tuikit::property_grid` (ADR 0187). An unresolved-evidence row is a `LinkRow` with no inspect intent: CSP-788 made `Enter` there point at the Preview, which shows the evidence, and left opening it whole with `o` to this story (operator review, CSP-788: give the row an inspect intent, a `tuikit::property_grid` change, and name `o` in `App::refusal_text`). The inspector is a host overlay on the modal stack (a `Pager` content like the value modal, ADR 0178), and promoting evidence to a declared link confirms through a `Dialog` (house rule 3, ADR 0172).
<!-- SECTION:NOTES:END -->
