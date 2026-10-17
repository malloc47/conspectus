---
id: CSP-617
title: Fix `pin adopt` so it finds live tmux sessions
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: high
type: bug
ordinal: 675000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus pin adopt <id> <mux-name>` fails for every live tmux session. It builds the prefixed id form (`tmux:<name>`, or `tmux:<socket>:<name>`) and compares it with `MuxSessionNode.native_id` (`src/cli/pin.rs:635-645`), but discovery stores the bare session name in that field and keeps the prefixed form on `MuxSessionNode.id` (`src/discovery/tmux/mod.rs:1558-1560`). Reproduced on 2026-10-16 against an isolated tmux server: with session `adoptme` running, discovery emitted `id = tmux:adoptme, native_id = adoptme`, and `conspectus pin adopt verify-pin adoptme --harness codex --cwd <dir>` exited 1 with "no live mux with native_id `tmux:adoptme`". The only adopt integration test (`tests/cli_pin.rs::pin_adopt_fails_when_mux_not_live`) covers the failure path, so nothing exercised a successful adopt against nodes shaped the way discovery emits them. Adopt is the documented way to bring an existing tmux session under a pin (ADR 0057).

- Scope: match the requested mux against the field that carries the prefixed form, for default and named sockets, and leave the stored pin's `mux` fields unchanged. The TUI adopt flow takes a different code path; check that it applies the same matching rule.
- Tests: a CLI-level test that adopts a mux whose node has the shape discovery emits (bare `native_id`, prefixed `id`) and asserts the pin is written; keep the not-live failure test.
- Manual checks: against a scratch tmux server (`TMUX_TMPDIR` pointed at a short temp directory, `HOME` and the XDG dirs at a temp tree), run `tmux new -d -s adoptme`, then `conspectus pin adopt p adoptme --harness codex --cwd <dir>` writes the pin.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Running `conspectus pin adopt` against a live tmux session on the default socket writes the pin
- [ ] #2 A test covers a successful adopt against mux nodes shaped the way discovery emits them
- [ ] #3 The CLI and TUI adopt paths apply the same mux matching rule
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
