---
id: CSP-910
title: >-
  TUI: the mux that hosts this TUI previews its agent transcript under
  mux_preview = transcript
status: To Do
assignee: []
created_date: '2026-11-25 18:58'
labels:
  - tui
  - config
dependencies: []
references:
  - src/tui/preview/mod.rs
  - >-
    docs/adr/0256-tui-a-live-mux-can-preview-the-transcript-of-the-agent-it-hosts.md
priority: low
type: enhancement
ordinal: 985000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mux a TUI runs in previews nothing: its row says `no mux target for this row`, because capturing its pane would show the TUI inside itself. CSP-908 added `[tui] mux_preview = "transcript"`, which makes a live mux preview the transcript of the agent it hosts, and kept this exception as it was (ADR 0256, decision 7). That recursion hazard does not apply to a transcript, so an operator who runs an agent in the same tmux session as Conspectus, and opted in to transcripts, still gets an empty Preview on that one mux where every other mux shows its agent.

It was left out of CSP-908 on purpose: it changes a documented rule (ADR 0112, ADR 0253, ADR 0256) for a case that opting in does not need, so it should be asked for. The operator asked for it on 2026-11-25.

Two points to settle in the work. Under `pane`, nothing can change, since the pane is the recursion. Without a live agent whose harness the viewer can read, the mux has nothing safe to show and keeps its placeholder, which should still say something true.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Under `mux_preview = "transcript"`, the mux that hosts this TUI previews the transcript of the session `v` opens on it, in every place a live mux previews: the Mux view row, the Pinned view row for a pin on it, a drilled-in node, and a Related neighbor
- [ ] #2 Without a live agent whose harness the viewer can read, and under `pane`, that mux previews nothing as it does today, with a placeholder that is still accurate
- [ ] #3 Attaching to the mux that hosts this TUI is still refused, and a pane of it is still never captured
- [ ] #4 A new ADR records the change to the rule and amends ADR 0256; the operator guide and design.md say so
- [ ] #5 Tests cover the source in each place above, both sparse cases, and a rendered frame; `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
