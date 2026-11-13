---
id: CSP-852
title: Gate a split pane's hook record against its own pane's harness
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 12:52'
updated_date: '2026-11-13 20:36'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/research/06-internal-audit.md
priority: low
type: bug
ordinal: 937000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Attribution ignores a hook record when the mux's *active* pane runs a different harness (`pane_running_harness` in `src/discovery/hook_sidecar.rs` reads `MuxSessionNode.active_pane_command`). Since the spool keeps one record per pane (CSP-818), two agents of different harnesses in split panes of one tmux session both keep records, but whichever pane is not active is marked Ignored ("pane now running ...") against the other pane's command, so only one of them is attributed. The graph has no pane entity and the mux node describes only the active pane, so the gate cannot see the record's own pane. Found while fixing the spool keys (CSP-818); see the internal audit section 1.8.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A hook record from a non-active pane is not ignored because the active pane runs another harness
- [x] #2 A record whose own pane now runs another harness is still ignored
- [x] #3 Two agents of different harnesses in split panes of one tmux session are both attributed after a refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Mux probe: after a successful tmux session listing, list every pane of the server once (`list-panes -a -F '#{pane_id}\t#{session_name}\t#{pane_current_command}'`, already a sanctioned read) and record each pane's session and program on the probe (`MuxProbe.panes`, by pane id). A failed listing records none. Bump the snapshot format.
2. Hook sidecar: gate a record against the harness its own pane runs. When the record names a pane and its server's probe listed panes, read that pane's program; a pane the listing does not show is not gated. A record that names no pane, or a probe with no pane listing, keeps the old gate on the mux's active pane.
3. Tests: probe parses the pane listing (and tolerates junk / a failed call); gate unit tests for a non-active pane beside another harness (attributed), a record whose own non-active pane now runs another harness (ignored), the fallbacks; a discovery test with a fake tmux whose split panes run Claude Code and Codex, both attributed after a refresh.
4. Docs: new ADR for the pane listing and the gate, amendment notes on ADR 0028 and 0146 where they name the active-pane gate, operations guide, CHANGELOG, capabilities manifest ADR citation.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The gate needed a view of non-active panes, so the tmux probe now lists every pane once per server with sessions (`list-panes -a`, id, session, program) and keeps it on the probe (ADR 0220). Fallback to the active pane only when the record names no pane or no listing exists. Validation: new unit tests in hook_sidecar_tests (non-active pane beside another harness attributed; own non-active pane now running Codex ignored; unlisted pane not gated; no-pane record still uses the active pane; two harnesses in split panes both Active whichever pane is active), tmux_tests (pane parser, probe records panes, failed listing, no call without sessions), and discovery_tests (fake tmux with Codex in %1 and Claude Code in %2: both resolved to the mux). checks.sh green; full suite 5,657 passed, 0 failed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A hook record is now gated against the harness its own pane runs, not the mux's active pane. The tmux probe lists every pane of each server that has sessions (`tmux list-panes -a`, one more call per mux cycle, already a sanctioned read) and records each pane's session and program on the probe (`MuxProbe.panes`, in graph.bin only; snapshot format 15). A record naming a listed pane is judged by that pane's program; a pane the listing does not show gates nothing; a record with no pane, or a probe without a listing, keeps the active-pane gate. ADR 0220 records it, with amendment notes on ADRs 0028 and 0146; operations guide, design, CHANGELOG, and the manifest's tmux citation updated.

Verified: unit tests for the gate (split pane beside another harness attributed, own non-active pane now running Codex ignored, fallbacks), probe tests for the listing, and a discovery test where Codex and Claude Code in split panes of one fake tmux session both resolve to the mux. fmt, clippy -D warnings, rustdoc -D warnings, git diff --check, and the full cargo test suite (5,657 passed, 0 failed) pass; tuikit is untouched.
<!-- SECTION:FINAL_SUMMARY:END -->
