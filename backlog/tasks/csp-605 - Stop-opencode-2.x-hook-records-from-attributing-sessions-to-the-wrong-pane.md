---
id: CSP-605
title: Stop opencode 2.x hook records from attributing sessions to the wrong pane
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 02:58'
updated_date: '2026-10-18 06:21'
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
- [x] #1 Hook records written from an opencode 2.x shared service never yield `linked_to_mux` candidates from inherited tmux context
- [x] #2 Opencode 1.x hook records keep attributing exactly as before
- [x] #3 The way 2.x service records are detected is documented, with the evidence CSP-602 found for it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- Verified: the service (`opencode serve --service`) inherits `TMUX`/`TMUX_PANE` from the client that started it, and server plugins run inside it, so every record it writes names that first pane.
- Detection: the hook writer's harness ancestor (first `opencode` comm walking up) is the service; its argv has `serve` as the subcommand. A TUI plugin's child has the TUI client as that ancestor instead.
- Stripping the tmux context is not enough: `discovery::hook_sidecar::find_mux` falls back to pid and then cwd, so a pane-less service record would still bind at hook rank to any pane sharing the cwd. Service-context records must produce no mux link at all.

Detection (ADR 0126): `conspectus hook write` reads the command line of the harness ancestor its pid walk already finds and asks that harness's `RuntimeSignature::is_background_process` (from CSP-608: opencode `serve`/`acp`). When it matches, the record gets the new provider-neutral `HookRecord.background_harness` flag and no tmux context. Evidence from CSP-602 (ADR 0125): the 2.x service is `opencode serve --service`, inherits the first client's TMUX/TMUX_PANE, and hosts server plugins.
Sidecar: `apply_hook_records` skips background-harness records before `find_mux`, because stripping tmux alone would still let the pid and cwd fallbacks bind them at hook rank. The records stay in the store.
Manual check: with a sandboxed 2.x service started from pane A and a TUI in pane B, a server-side probe plugin ran the real `conspectus hook write opencode` for pane B's new session. The record carried `background_harness: true`, pid = service, ppid = pane A's TUI, and no tmux. `conspectus graph` showed no hook_sidecar link for the session; it resolved to pane B through activity evidence.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Hook records written from a harness's background process are now marked `background_harness` by the writer (detected from the harness ancestor's command line through the adapter's runtime signature; for opencode 2.x, the shared `opencode serve --service`), carry no tmux context, and never produce mux links in the hook sidecar, including through the pid and cwd fallbacks. 1.x in-pane records attribute as before. ADR 0126 records the detection and the 2.x attribution plan; design.md notes it.
<!-- SECTION:FINAL_SUMMARY:END -->
