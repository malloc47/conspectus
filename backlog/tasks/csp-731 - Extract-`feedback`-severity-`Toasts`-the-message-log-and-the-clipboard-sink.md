---
id: CSP-731
title: >-
  Extract `feedback`: severity, `Toasts`, the message log, and the clipboard
  sink
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-721
  - CSP-722
  - CSP-726
references:
  - docs/adr/0056-tui-clipboard-backend-osc52.md
  - docs/adr/0105-tui-message-log.md
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/adr/0152-tui-framework-dependency-ownership.md
priority: medium
type: enhancement
ordinal: 817000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Feedback has three channels with no shared type. Toasts are always green because `ratatui-comfy-toaster` hard-codes the border and ignores `theme.success`, and the status line paints every message in the warning colour, so failures look like successes. The message log is a bounded repeat-collapsing list wired to a domain `LogTarget` and `CommandRecord`. The clipboard writes OSC 52 to process stdout, so tests cannot capture it (inventory slices 06 and 07).

Scope: Create `tuikit::feedback` with `Feedback { severity, text, ttl }` (info, success, warning, error), `Toasts` as in-tree code in place of `ratatui-comfy-toaster` (ADR 0152) with a theme-honoring border and longer errors, the generic bounded repeat-collapsing log with an unseen counter (the `LogTarget` and `CommandRecord` payload stays in Conspectus as the log's item type), a status-line renderer that styles by severity, and the OSC 52 clipboard taking a host-supplied `Write`. Remove `ratatui-comfy-toaster`.

TUI changes:
- Consistency wins: failures stop looking like successes; one feedback type behind the status line, toast, and message log; clipboard writes are testable
- Regressions or behavior changes: toast and status-line colours change for warnings and errors; errors stay on screen longer

Tests and checks: Toast, log, and status-line tests per severity. The clipboard test captures the escape sequence through the sink. `--snapshot` of each severity.

Blockers: The theme, glyph, and frame extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Feedback` has four severities, and the status line, toast, and message log all style from the theme by severity
- [ ] #2 `Toasts` is in-tree code with a theme-honoring border, and `ratatui-comfy-toaster` is removed from the dependency tree
- [ ] #3 The bounded repeat-collapsing log is generic and Conspectus's `LogTarget` and `CommandRecord` are its item payload
- [ ] #4 The clipboard writes to a host-supplied `Write` and a test captures the sequence
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
