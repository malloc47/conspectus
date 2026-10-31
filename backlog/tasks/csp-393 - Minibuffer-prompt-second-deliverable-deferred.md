---
id: CSP-393
title: 'Minibuffer prompt (second deliverable, deferred)'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
updated_date: '2026-10-31 08:15'
labels:
  - h-cmd
milestone: m-17
dependencies:
  - CSP-392
  - CSP-241
ordinal: 518000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a bottom-anchored single-line prompt bar that can host
  any text-command interaction — rename, search, filter, command
  palette entry — without spawning a new modal overlay. The
  minibuffer is the *unified* prompt surface; individual commands
  declare their prompt content and completion set. Emacs conventions:
  `M-x` opens the minibuffer in command-palette mode,
  `C-g`/`Esc` cancels. Tab completion cycles candidates. History
  persists per-command-type for the session lifetime. The existing
  rename input (`CSP-241`) should route through the minibuffer
  rather than its own modal once this lands; the search overlay
  (`CSP-193` / `/`) should use it too. Do not replace the viewer
  modal or graph-export dialogs — they need more screen real estate.
- Tests: snapshot tests for empty prompt, text entry with completion
  candidates, history navigation (`M-p`/`M-n`), cancellation, and
  confirm. Reducer tests for per-command-type routing (command
  palette dispatch, rename dispatch, search dispatch).
- Manual checks: open minibuffer, type a partial command, tab-
  complete, execute; then open rename via minibuffer and confirm the
  alias update flow works identically to the current modal path.
- Blockers: `CSP-392`, `CSP-241` (rename flow is the first
  non-palette consumer).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CMD-004`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the minibuffer is a host of `tuikit::text_prompt::TextPrompt<M>` (ADR 0166). The prompt's page (`crates/tuikit/docs/controls/text_prompt.md`, "Seams left open") names the three pieces: a minibuffer host (the same key handling and `PromptResult`, drawn in a one-row bar instead of a modal frame), completion and history (candidates under the field, taking `Tab` and the arrows), and live validation. Rename already runs on `TextPrompt`. Tab completion is the roadmap's general completion source, generalizing `PathField`'s `PathSource` (ADR 0167). Check `M-x`, `M-p`/`M-n`, and `C-g` against the key model (ADR 0160) and the text-input table.
<!-- SECTION:NOTES:END -->
