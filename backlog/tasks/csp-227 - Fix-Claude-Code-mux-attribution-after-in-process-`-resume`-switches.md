---
id: CSP-227
title: Fix Claude Code mux attribution after in-process `/resume` switches
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-217
  - CSP-218
  - CSP-219
  - CSP-226
ordinal: 270000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: live testing showed a Claude Code process running in
  tmux with argv
  `claude --resume 926c6991-9494-48ee-9d63-a98f4b4959d0`, while
  the pane's `/usage` screen reported current session id
  `2f11bd94-da81-4c5c-975d-a29dcdb3cda0`. Conspectus therefore
  linked the tmux session to the launch/resume id (`926c6991`)
  via `active_pane_command_session_match`, even though the active
  transcript and recency belonged to `2f11bd94`. The operator
  likely entered the older tmux session and then used Claude
  Code's in-app `/resume` to switch sessions. The process argv did
  not update, so command-line resume evidence became stale.
- Scope: refine Claude Code session ↔ mux attribution so
  command-line `--resume <session>` is treated as launch evidence,
  not definitive current-session evidence, when a stronger
  current-session source exists. Investigate, in order:
  non-mutating Claude control/state sources from `CSP-219`;
  hook/sidecar payloads from `CSP-223` / `CSP-226`;
  read-only state/database/file evidence from `CSP-217` /
  `CSP-218`; and only then carefully-scoped pane scraping of
  already-visible status surfaces such as `/usage`. Do not inject
  `/usage`, `/status`, or any slash command into the pane as part
  of discovery.
- Desired behavior: when launch argv names session A but stronger
  current-session evidence names session B in the same running
  Claude process, emit or select the `LinkedToMux` candidate for B
  and demote A to launch-history evidence. The TUI should then show
  the attached mux indicator beside B, and B's recency should not
  look like an unattached background write.
- Tests: fixture a mux node whose `active_pane_start_command`
  contains `--resume A` and a stronger Claude-current-session
  evidence source names B; assert resolver selects B and does not
  keep A as the preferred mux relationship. Add a regression for
  the no-stronger-evidence case where argv remains usable. Add a
  TUI row-tree assertion that the mux indicator follows B.
- Manual checks: reproduce with a live Claude Code tmux session:
  start or attach to session A, switch in-app to session B with
  `/resume`, verify the pane reports B as current, then confirm
  `conspectus graph --format json` and `conspectus tui` link the
  mux to B.
- Related: `CSP-136` (current argv/fd/process evidence),
  `CSP-217` (session-file activity correlation),
  `CSP-218` (read-only harness state), `CSP-219`
  (control-plane audit), `CSP-222` (terminal-injection
  policy), `CSP-223` / `CSP-224` /
  `CSP-226` (Claude hook sidecar path), `CSP-175`
  (ambiguous mux picker if evidence remains unresolved).
- Blockers: no hard blocker for documenting/demoting argv
  semantics; a definitive fix likely depends on one of
  `CSP-217`, `CSP-218`, `CSP-219`, or
  `CSP-226`.
- **regression slice landed**: resolver tests cover stronger
  current-session evidence beating launch argv and launch argv
  remaining usable without a current-session source. Hook-sidecar
  tests cover fresh Claude-current-session evidence overriding a
  stale `active_pane_command_session_match` for the same mux, plus
  live-validation fallout where a fresh Claude session exists in
  hook state before its transcript file exists on disk.
- **codex-side fix landed via `CSP-218` / ADR 0048**: the
  codex log linker resolves the active thread for each live codex
  pid by parsing `logs.process_uuid` (`pid:<os_pid>:<uuid>`) and
  demotes stale `active_pane_command_session_match` candidates for
  the same mux, closing the codex equivalent of this drift class
  without requiring hooks.
- **Claude-side fix landed via `CSP-226` + ADR 0028 hook
  sidecar stack** and confirmed in-the-wild on 2026-05-30. Live
  `conspectus graph --format json` against the development host
  showed two concurrent Claude panes with `claude --resume A` argv
  whose hook-sidecar records had reported the operator's
  post-`/resume` current session B; in both cases the cross_link
  `active_pane_command_session_match` for A was already
  `Overridden` by a fresh `hook_session_path_match` link to B with
  reason "fresh hook sidecar current-session evidence", and on one
  of the panes 10 older orphaned hook records for other sessions
  were correctly demoted by the freshest record via "superseded by
  fresher hook sidecar record for same pane". Three other Claude
  panes whose argv id matched the hook id produced corroborating
  candidates that did not need the override path. No code change
  was needed for closure; the resolver tests and ADR 0028 hook
  sidecar machinery already shipped the fix.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-015`
