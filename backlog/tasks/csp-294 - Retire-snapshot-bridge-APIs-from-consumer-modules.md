---
id: CSP-294
title: Retire snapshot bridge APIs from consumer modules
status: To Do
assignee: []
created_date: '2026-05-28 02:52'
updated_date: '2026-10-31 08:15'
labels:
  - p10-fu
milestone: m-15
dependencies: []
ordinal: 486000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: remove compatibility entry points that accept
  `GraphSnapshot` only to materialize an in-memory SQLite database
  before rendering or building view-models. Candidate APIs include
  `output::table::render`, `output::table::render_with`,
  `output::node_show::resolve_node_id`,
  `output::node_show::render_node_show`,
  `tui::detail::build_node_detail`, and the snapshot-taking
  `tui::rows::sessions::build_sessions_tree` test bridge. Keep
  producer-side helpers such as `query::materialize_snapshot` when
  they still serve discovery/resolver fixtures or one-shot cold
  builds.
- Tests: update fixture-heavy tests to materialize SQLite explicitly
  and call the `*_conn` entry points; full suite stays green.
- Manual checks: review `docs/library-api.md` and public exports so
  rendering examples use connection-backed APIs only.
- Blockers: P10 has landed and downstream tests/users have had a
  chance to move to `render_conn` / `render_with_conn`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P10-FU-001`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `tui::detail::build_node_detail` went with the sectioned detail model (`src/tui/detail.rs`, deleted by CSP-710); the right pane's explorer reads the graph through its source (`src/tui/explorer/source.rs`, ADR 0188) and has no snapshot-taking entry point. Drop it from the candidate list; the other candidates are unchanged by the framework work.
<!-- SECTION:NOTES:END -->
