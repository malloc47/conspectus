---
id: CSP-731
title: >-
  Extract `feedback`: severity, `Toasts`, the message log, and the clipboard
  sink
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 23:39'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (snapshot build at HEAD) of the toast, status line, and Messages overlay at 100x30 and 60x20, extending snapshot mode with dev-only flags that post a toast, set the status line, and seed log entries of a chosen severity.
2. Add `tuikit::feedback`: `Severity` (info, success, warning, error; color/style/mark/default lifetime from the theme), `Feedback { severity, text, ttl }`, the status-line span helper, `Toasts` (in-tree queue/stack with per-toast lifetime, bottom-centered bordered rendering; border from the severity token, text from a new `toast_text` token; same placement, 1.5 s, and look as before for success), the generic `Log<T>`/`Entry<T>` (bounded, repeat-collapsing, unseen counter), and `clipboard::copy` writing OSC 52 to a host-supplied `Write`. Unit tests per severity; doc page `docs/controls/feedback.md`.
3. Host: `messages.rs` keeps `LogTarget`/`CommandRecord` as the log item payload (`Record`), `LogLevel` becomes `Severity`; `clipboard.rs` passes stdout to the framework; `widgets/toast.rs` and `ratatui-comfy-toaster` go; App holds `Toasts`.
4. Status line: `status_message` holds a `Feedback`; `Msg::SetStatus(Option<Feedback>)`; header draws by severity. Audit every status site and assign a severity (refusals/failures warning or error, confirmations success, hints info).
5. Toasts: `Effect::Toast(Feedback)`, `Msg::Toast(Feedback)`, `post_toast(Feedback)`; give every call site a severity.
6. ADR 0163, ADR index, design/operations/changelog notes, operator-review entries; after frames and comparison; full checks.
<!-- SECTION:PLAN:END -->
