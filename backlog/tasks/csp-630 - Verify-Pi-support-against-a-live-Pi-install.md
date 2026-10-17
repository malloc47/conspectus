---
id: CSP-630
title: Verify Pi support against a live Pi install
status: To Do
assignee: []
created_date: '2026-10-17 18:23'
labels:
  - h-pi
milestone: m-22
dependencies: []
priority: medium
type: task
ordinal: 688000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Pi harness work (CSP-610 to CSP-615) was built and tested without Pi installed: against checked-in fixtures, upstream docs, and a stubbed extension API under Node. Several behaviors have never been exercised against a real Pi, and one design assumption is unconfirmed. ADR 0121 assumes `/proc/<pid>/comm` reads `pi` for a running Pi, because Node's process title also sets the thread name on Linux. Hook pid resolution depends on that. If it is wrong, Pi hook records carry no pid and the sidecar's liveness check never applies to them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A short pi session in a repo appears in conspectus with the right cwd, repo, title, and activity; a --no-session run leaves nothing behind
- [ ] #2 /fork and /clone children show their parent in the sessions graph
- [ ] #3 A pi pane in tmux is attributed to its session; resume from the TUI and a pin launch both start pi correctly
- [ ] #4 After conspectus hook init pi, /new and /resume inside one pi move the pane attribution, and the hook record carries the pi process pid (comm reads pi); if it does not, the extension passes its pid and ADR 0121 is amended
- [ ] #5 The extension loads under the installed pi version without errors, and the viewer shows the same conversation as pi --session <id>, including an Esc-aborted exchange hidden by default
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
