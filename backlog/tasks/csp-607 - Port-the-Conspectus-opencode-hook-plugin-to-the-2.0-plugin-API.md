---
id: CSP-607
title: Port the Conspectus opencode hook plugin to the 2.0 plugin API
status: To Do
assignee: []
created_date: '2026-10-17 02:59'
updated_date: '2026-10-18 05:58'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
references:
  - 'https://opencode.ai/v2/docs/migrate-v1/'
priority: medium
ordinal: 656000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`plugins/opencode-hook` (CSP-229, ADR 0049) is a 1.x plugin built against `@opencode-ai/plugin` (peer `>=1.14.0`). It subscribes to `session.created`, `session.updated`, `session.status`, `session.idle`, and `session.compacted`. The V1 migration guide says V1 plugin implementations do not run in V2, and the config key changed from `"plugin": [...]` to `"plugins": [{ "package": ..., "options": ... }]`. After upgrading, an operator's plugin silently stops loading, and the hook sidecar stops receiving opencode observations. The README's install instructions (`opencode plugin <path>`, `~/.config/opencode/config.json`) may also be wrong for 2.x.

What the records may carry once ported depends on CSP-605 (no inherited pane context) and CSP-606 (whether events can identify the client).

- Scope: a plugin that loads under 2.x and forwards the equivalent session lifecycle observations to `conspectus hook write opencode`; decide in ADR 0049, or an amendment to it, whether one package serves both majors or 1.x keeps its own build; update the README's install and verify steps for 2.x; keep the plugin non-blocking when the `conspectus` binary is missing.
- Tests: typecheck and build against the 2.x plugin package; a test or fixture showing the written record shape for each forwarded event.
- Manual checks: in a sandboxed 2.x install, the plugin loads, and a new session's observation reaches the hook store (`conspectus` hook query per the README).
- Blockers: CSP-602.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The plugin loads in opencode 2.x and forwards session create, update, status, idle, and compaction observations (or their 2.x equivalents)
- [ ] #2 The README documents installation and verification for 2.x, plus 1.x while it stays supported
- [ ] #3 ADR 0049 is amended (or superseded) to record the 2.x distribution decision
- [ ] #4 A missing or failing `conspectus` binary does not disrupt the opencode session
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- 2.x plugins are `{id, setup(ctx)}` from npm `@opencode/plugin`, with separate `server` and `tui` entrypoints. Only the `tui` entrypoint sees the pane, so the port is a TUI plugin that forwards the routed session's lifecycle and route changes; a `server` entrypoint would only produce pane-less records.
- Local install is `"plugins": [{"package": "<dir>"}]` in opencode.jsonc (`opencode plugin add` takes only npm/Git specifiers). The host resolves `<dir>/server`, then `<dir>/index`, and `<dir>/tui`, so the 2.x build must live in a directory without the 1.x entrypoint.
<!-- SECTION:NOTES:END -->
