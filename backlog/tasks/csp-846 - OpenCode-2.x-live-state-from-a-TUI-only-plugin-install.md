---
id: CSP-846
title: 'OpenCode 2.x: live state from a TUI-only plugin install'
status: To Do
assignee: []
created_date: '2026-11-12 11:17'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 931000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On opencode 2.x the plugin's live state comes from its server entrypoint, which runs in the shared service and has to be listed in `opencode.jsonc` (ADR 0206 decision 23, amending ADR 0126). An operator who lists `dist/v2` only in `cli.json`, as plugin 0.2.0 asked, loads the TUI entrypoint alone: pane attribution works, but no session reports live state. Research note 03 (section 5.2, option A) describes a fallback: the TUI entrypoint sends snapshots for its routed session's family from its own stores (`ctx.data.session.status`, `session.permission.list`, `session.form.list`), accepting that sessions no client shows are missed. Two writers for one session would alternate in the store (different plugin instances), so the fallback must stay quiet while a server entrypoint reports. Found while building the OpenCode slice (CSP-816).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With only cli.json listing the plugin, a 2.x client's routed session and its child sessions report live state
- [ ] #2 With the server entrypoint loaded, the TUI entrypoint sends no live-state snapshots
- [ ] #3 Event scripts in tests/fixtures/hooks/opencode-live cover the fallback, replayed by the plugin tests and the Rust tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
