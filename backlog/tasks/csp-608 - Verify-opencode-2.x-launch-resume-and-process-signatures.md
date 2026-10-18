---
id: CSP-608
title: 'Verify opencode 2.x launch, resume, and process signatures'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 02:59'
updated_date: '2026-10-18 06:13'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
priority: medium
ordinal: 657000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Several opencode assumptions sit outside the storage layer, and 2.0's single background service could invalidate them:
- `HarnessAdapter::launch_argv` is `opencode` and `resume_argv` is `opencode --session <id>`. Pins (ADR 0057, 0058, 0098) and `mux launch` (ADR 0096) depend on these. A changed flag makes every opencode pin resume fail; ADR 0103's failed-start fallback would then quietly launch fresh every time.
- The runtime signature in `src/discovery/harness/opencode.rs` and `src/discovery/cross_link.rs` classifies pane processes, including opencode subagent processes as `Subagent`. In 2.x, subagents and shell commands run inside the service, and the service itself is a long-lived `opencode` process that is not anyone's interactive session. If it is classified as a human agent, it can create spurious process evidence.
- Pin launch's `StaleMux` guard (ADR 0102, `mux_hosts_harness`) detects a running harness through pane-process evidence, so it needs to recognize a 2.x TUI client as the harness running.

- Scope: confirm or update launch and resume argv for 2.x, keeping 1.x working if the CLI differs; classify the 2.x service and client processes correctly in the runtime signature; make sure `mux_hosts_harness` still recognizes a 2.x TUI in a pane; update `docs/capabilities.toml` if any launched program or argv shape changes (ADR 0117).
- Tests: argv tests for both majors if they differ; runtime-signature tests for 2.x client, service, and (if they exist) subagent process command lines.
- Manual checks: in a sandboxed 2.x install, `conspectus pin launch` resumes a recorded session, `mux launch opencode` starts a TUI, and the service process does not appear as an extra human-agent process in `conspectus graph`.
- Blockers: CSP-602.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Resuming an opencode 2.x pin reopens the recorded session rather than falling back to a fresh launch
- [x] #2 The opencode 2.x background service process is not classified as a human-agent runtime process
- [x] #3 A 2.x TUI client in a pane counts as the pin harness running for the `StaleMux` attach guard
- [x] #4 Opencode 1.x launch, resume, and process classification are unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- Launch (`opencode`) and resume (`opencode --session <id>`) argv are unchanged in 2.x; capabilities.toml needs no change.
- `--session <unknown id>` opens an empty TUI whose first prompt creates the session under that exact id, so ADR 0103's failed-start fallback no longer fires for opencode 2.x.
- The service starts as a child of the first TUI (so it sits in that pane's process tree until the TUI exits) with argv `opencode serve --service`; classify `serve` (and stdio-driven `acp`) as background.
- 2.x subagents and shell tools run in the service, so there is no 2.x subagent process to classify. A 2.x TUI client is a plain `opencode` process in its pane, so `mux_hosts_harness` sees it.

Launch and resume argv are unchanged for 2.x (`opencode`, `opencode --session <id>`), so `HarnessAdapter` argv and docs/capabilities.toml stay as they are. ADR 0125 records that 2.x `--session <unknown id>` creates the session under that id, so ADR 0103's failed-start fallback no longer fires for opencode 2.x.
Classifier: `opencode_is_background_process` treats a first non-flag argument of `serve` or `acp` as a headless server. The cross_link role helpers were renamed from `is_claude_background_process` / `is_opencode_subagent_process` to `is_background_process` / `is_subagent_process`, since they delegate to any adapter's signature.
Manual checks, in a sandbox with a private tmux socket dir and the 2.x binary named `opencode`: `pin create` + `pin launch --no-attach` started `opencode`; after a prompt the pin bound to the new session; killing the mux and relaunching printed "resuming recorded session" and ran `opencode --session <id>`, whose TUI router showed that session. `pin launch` against the live mux reported it already bound (no StaleMux). With the service started by the pin's TUI (so a child in its pane), `conspectus graph` before this change listed the service as a second `human_agent` process in the pane; after it, only the TUI.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Verified that opencode 2.x keeps the launch and resume argv Conspectus uses, end to end through `pin launch` resume in a private tmux. The 2.x shared service (`opencode serve --service`), which starts as a child of the first TUI and so sits in that pane, plus `opencode serve` and `opencode acp`, are now classified as background processes, so they no longer appear as extra human-agent processes or count as the harness running; a 2.x TUI client still does. 1.x classification is unchanged.
<!-- SECTION:FINAL_SUMMARY:END -->
