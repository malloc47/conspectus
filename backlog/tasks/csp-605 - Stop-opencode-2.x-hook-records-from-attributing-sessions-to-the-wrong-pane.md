---
id: CSP-605
title: Stop opencode 2.x hook records from attributing sessions to the wrong pane
status: To Do
assignee: []
created_date: '2026-10-17 02:58'
updated_date: '2026-10-17 03:03'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
priority: high
ordinal: 654000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Opencode hook records come from the Conspectus opencode plugin (CSP-229, ADR 0049), which shells out to `conspectus hook write opencode`. The writer takes its mux context from its own `$TMUX`/`tmux display-message` and finds the harness pid by walking parent processes to the first `opencode` ancestor. That was right in 1.x, where the plugin ran inside each TUI process, in that TUI's pane. In 2.x plugins run inside the single shared background service. The writer would see the service's environment, meaning the pane of whichever client started the service, or none. Every opencode session would then be stamped with that one pane, at hook-match rank, outranking the correct activity and process evidence. A wrong high-confidence link does more damage than a missing one.

This task only makes 2.x hook evidence safe. Restoring good attribution is the separate design task that depends on this one.

- Scope: recognize hook records written from an opencode 2.x service context, and make sure they never produce `linked_to_mux` evidence from inherited tmux context, whether in the writer, in `discovery::hook_sidecar`, or both. Records can still supply session observations with no mux link. Leave 1.x records unchanged.
- Tests: hook-writer and hook-sidecar tests for a 1.x in-pane record (still attributes) and a 2.x service record carrying an inherited `TMUX` (no mux link); a resolver test showing that two 2.x sessions in different panes are no longer forced onto one mux.
- Manual checks: with a sandboxed 2.x service started from one pane and a second TUI in another pane, `conspectus node show` for the second pane's session shows no hook link to the first pane.
- Blockers: CSP-602.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Hook records written from an opencode 2.x shared service never yield `linked_to_mux` candidates from inherited tmux context
- [ ] #2 Opencode 1.x hook records keep attributing exactly as before
- [ ] #3 The way 2.x service records are detected is documented, with the evidence CSP-602 found for it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
