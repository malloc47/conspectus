---
id: CSP-526
title: Atelier `exec claude` panes render as "No agent" in the TUI / CLI
status: Done
assignee: []
created_date: '2026-08-08 03:23'
labels:
  - h-harness-atelier
milestone: m-18
dependencies: []
ordinal: 549000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Progress (2026-10-02): the rendering half landed in `549acab`. An
  agentless mux row now carries `program_harness` from the pane's
  `mux_contains_process` evidence, falling back to a harness token in
  the start command or pin argv, so an `atelier exec claude` pane shows
  the claude badge instead of "no agent" or a black `atelier` chip. A
  stand-in wrapper (a parent named `atelier` spawning a child whose
  argv[0] is `claude`) confirmed the process walk attributes the
  harness at depth 1, so none of the fix directions below was needed
  for the badge.
- Symptom: launching claude via atelier's `atelier exec claude`
  convention produces a live claude process in the pane's tree, but
  Conspectus attributes no harness to the mux — the row renders as
  "No agent" and no `AgentSession` is linked. Direct `claude` launches
  are unaffected.
- Suspected surface: process-tree harness attribution
  (`process_command_harnesses` in `src/discovery/cross_link.rs:1472`)
  keys off the argv[0] basename of each descendant pid, lowercased and
  matched against each adapter's `RuntimeSignature.process_command_basenames`.
  The claude adapter lists `["claude", "claude-code"]`
  (`src/discovery/harness/claude_code.rs:97`). If `atelier exec`
  inserts a wrapper whose descendant argv[0] is not one of those
  tokens (e.g. a nix-store path with an unexpected basename, a shim
  binary, a shell-form command line, or a depth exceeding
  `PROCESS_TREE_MAX_DEPTH`), attribution silently misses.
- Investigation (needs live repro): from an operator box with a live
  `atelier exec claude` pane, capture `ps -eo pid,ppid,command
  --forest` rooted at the pane pid, plus `/proc/<pid>/cmdline` for
  each descendant. Compare against the matcher above to identify the
  concrete miss (argv shape, depth, or command basename).
- Likely fix directions (pick after diagnosis): (a) add the observed
  wrapper basename to `process_command_basenames`; (b) fall back to
  `command_substrings` when basename match fails; (c) raise
  `PROCESS_TREE_MAX_DEPTH`; (d) special-case an `atelier` adapter that
  delegates attribution.
- Tests: once the miss shape is known, add a cross_link fixture that
  reproduces the process tree and asserts claude attribution.
- Blockers: needs live process-tree data. Deferred until repro
  provided.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-10-02): the operator confirmed with a live `atelier
exec claude` pane that the session links to its mux as well. Closed.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-HARNESS-ATELIER-001`
