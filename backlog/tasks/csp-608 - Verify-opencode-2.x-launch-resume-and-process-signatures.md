---
id: CSP-608
title: 'Verify opencode 2.x launch, resume, and process signatures'
status: To Do
assignee: []
created_date: '2026-10-17 02:59'
updated_date: '2026-10-17 03:03'
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
- [ ] #1 Resuming an opencode 2.x pin reopens the recorded session rather than falling back to a fresh launch
- [ ] #2 The opencode 2.x background service process is not classified as a human-agent runtime process
- [ ] #3 A 2.x TUI client in a pane counts as the pin harness running for the `StaleMux` attach guard
- [ ] #4 Opencode 1.x launch, resume, and process classification are unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
