---
id: CSP-607
title: Port the Conspectus opencode hook plugin to the 2.0 plugin API
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 02:59'
updated_date: '2026-10-18 06:40'
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
- [x] #1 The plugin loads in opencode 2.x and forwards session create, update, status, idle, and compaction observations (or their 2.x equivalents)
- [x] #2 The README documents installation and verification for 2.x, plus 1.x while it stays supported
- [x] #3 ADR 0049 is amended (or superseded) to record the 2.x distribution decision
- [x] #4 A missing or failing `conspectus` binary does not disrupt the opencode session
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- 2.x plugins are `{id, setup(ctx)}` from npm `@opencode/plugin`, with separate `server` and `tui` entrypoints. Only the `tui` entrypoint sees the pane, so the port is a TUI plugin that forwards the routed session's lifecycle and route changes; a `server` entrypoint would only produce pane-less records.
- Local install is `"plugins": [{"package": "<dir>"}]` in opencode.jsonc (`opencode plugin add` takes only npm/Git specifiers). The host resolves `<dir>/server`, then `<dir>/index`, and `<dir>/tui`, so the 2.x build must live in a directory without the 1.x entrypoint.

Layout: one package serves both majors. The 1.x plugin stays at the package root (`src/index.ts` → `dist/index.js`); the 2.x TUI entrypoint is `src/v2/tui.ts` → `dist/v2/tui.js`; both use `src/write.ts` (the spawn helper, version 0.2.0). No 2.x server entrypoint: it would run in the shared service and only produce background-harness records (ADR 0126).
Install finding: 2.x lists TUI-only plugins in `$XDG_CONFIG_HOME/opencode/cli.json` (`"plugins": ["<dir>"]`); a directory without a server entrypoint listed in `opencode.jsonc` is ignored, and `opencode plugin add` takes only npm/Git specifiers. Found by testing layouts in the sandbox and reading the 2.x `plugin add`/`plugin list` code; ADR 0125 corrected accordingly.
The 2.x entrypoint is typed against `@opencode/plugin` 2.0.32 (new dev dependency; planted type errors were caught, so the types resolve) but has no runtime import (`Plugin.define` is an identity function).
Manual checks in the sandbox (2.x binary named `opencode`, private tmux): with `cli.json` pointing at `dist/v2`, pane A (`opencode --session X`) and pane B (new session) wrote records with their own pane ids and their own TUI pids; after `/new` plus a prompt in pane A its record followed the new session, and `conspectus graph` resolved each session to its own pane via hook_sidecar. With `CONSPECTUS_HOOK_BIN=/nonexistent` the TUI kept working with an intact screen. The refactored 1.x entrypoint still writes records under opencode 1.18.32.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Conspectus opencode plugin now ships a 2.x TUI entrypoint (`dist/v2`) alongside the unchanged 1.x root entrypoint. Installed through opencode 2.x's `cli.json`, it runs in each client's pane and forwards route changes (`tui.route`) and the 2.x lifecycle events (`session.created`, `session.renamed`, `session.status`, `session.idle`, `session.compacted`) for the session that client shows, through the shared non-blocking writer. ADR 0049 is amended with the 2.x distribution decision, and the README documents 2.x and 1.x install and verification.
<!-- SECTION:FINAL_SUMMARY:END -->
