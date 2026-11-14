---
id: CSP-848
title: Report opencode's live-state health in hook status
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 11:18'
updated_date: '2026-11-14 00:44'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 933000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus hook status` takes only claude-code, codex, and pi, so the live-state health line (receiving, silent, no-events, off) is not available for opencode, whose plugin reports live state since CSP-816; the operations guide points operators at `table sessions --columns +live` instead. The store entries alone can say receiving. Saying installed or off means reading opencode's plugin config (`opencode.json`, `opencode.jsonc`, `cli.json`; JSONC), which ties into the open question of a `hook init opencode` (research note 03, question 8).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 conspectus hook status opencode ends with the live-state health line
- [x] #2 It writes nothing and reads no opencode file beyond the plugin config it needs
- [x] #3 docs/operations.md documents it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `conspectus hook status opencode` (a status-only harness value; init and remove stay without opencode): read only the files that list plugins — user scope `$XDG_CONFIG_HOME/opencode/{opencode.json,opencode.jsonc,config.json,cli.json}`, project scope `opencode.json[c]` and `.opencode/opencode.json[c]` — as JSONC (comments and trailing commas dropped, no new dependency), and find entries naming the plugin (`.../opencode-hook[...]` paths or `@conspectus/opencode-hook`).
2. Print an overall line and one line per existing file (`installed` with what it loads: 1.x, 2.x, 2.x client only; `not-installed`; `unreadable`), a stderr note for a cli.json-only or doubled listing, then the live-state health line; its `off` message names the plugin instead of a `hook init` command.
3. Tests: unit tests of the JSONC reader and entry matching; a CLI smoke test with scratch config dirs that checks the lines, the health line, and that nothing is written; traced capability run of `hook status opencode`.
4. ADR 0230; operations guide (install, health, troubleshooting, OpenCode section), plugin README, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Choices: a status-only harness value (init/remove unchanged); files read are the ones research note 03 and ADR 0125 name for plugin lists (user: opencode.json, opencode.jsonc, config.json, cli.json; project: opencode.json[c] and .opencode/opencode.json[c]); no OPENCODE_CONFIG override or plugins directory is read. An entry counts when it is a path with an `opencode-hook` component or `@conspectus/opencode-hook`. Any listing counts as installed (cli.json now reports via the client fallback). The health line's off message names the plugin instead of a hook init command (print_health now takes the off hint).
Validation: cli::hook_opencode unit tests (JSONC, matching, per-file status, notes); cli_smoke hook_status_opencode_* (both scopes, labels, notes, receiving after a hook write, a file-tree comparison showing nothing written, `hook init opencode` still rejected); tests/capabilities.rs traces `hook status opencode` with a seeded cli.json (no writes); checks.sh green; just check 5691 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus hook status opencode` (user or `--scope project`) now reads where opencode's config lists the Conspectus plugin and ends with the live-state health line. It reads only the plugin-list files (user: `opencode.json`, `opencode.jsonc`, `config.json`, `cli.json` under `$XDG_CONFIG_HOME/opencode`; project: `opencode.json[c]` in the directory and `.opencode/`) as JSONC with a small comment and trailing-comma stripper (no new dependency), prints a scope line and one line per existing file (`installed` with `1.x`, `2.x`, or `2.x client only`; `not-installed`; `unreadable`), adds a stderr note for a cli.json-only or doubled listing, and its `off` message says to build and list the plugin since there is no `hook init opencode` (still not added; init/remove do not accept opencode). It writes nothing. ADR 0230; operations guide (installing, the OpenCode section, the health table, troubleshooting), plugin README, CHANGELOG updated.

Verified with unit tests of the reader and matcher, two CLI smoke tests (both scopes, every label, the notes, `receiving` after a `hook write opencode`, an unchanged file tree), the traced capability test running it against a seeded `cli.json`, `/tmp/csp-fu/checks.sh`, and `just check` (5691 passed, 1 skipped).
<!-- SECTION:FINAL_SUMMARY:END -->
