---
id: CSP-703
title: Make the session viewer help modal match the main TUI help
status: To Do
assignee: []
created_date: '2026-10-24 03:21'
labels:
  - tui
milestone: m-23
dependencies: []
priority: medium
type: task
ordinal: 779000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pressing `?` in the main TUI and pressing `?` inside the session transcript viewer open help modals that look and behave differently, so the two feel like separate programs.

As filed, the main TUI help (`HelpOverlayWidget` in `src/tui/widgets/help.rs`) is built on the shared `popup_frame::themed_popup` shell like the other TUI modals. It has a plain ` Help ` title, section headings, left-aligned keys in `panel_focus_accent`, descriptions that wrap to the modal width, scrolling when the content overflows, and a footer generated from `keybindings::overlay::HELP_KEYS`.

The viewer help (`draw_help_overlay` in `src/viewer/widget.rs`) builds its own `Block`. It has a `success`-colored border and a badge-styled ` viewer keybindings ` title. Its keys are right-aligned, bold, and painted in the codex harness color. It sizes itself to its content, never wraps or scrolls, has no section headings or footer, and its rows are a hand-written list kept apart from the viewer key table (`VIEWER_KEYS` in `src/tui/keybindings/overlay.rs`), so the two can drift.

The viewer has its own `Theme` (`src/viewer/theme.rs`) and is `pub(crate)`, so sharing the main help's code may need a small seam rather than a direct call. Pick whatever keeps the module boundary clean.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The viewer help modal uses the same frame as the main TUI help: border, title style, padding, and how it centers and sizes against the terminal
- [ ] #2 Key and description columns look the same in both: same key color and weight, same alignment, same key notation (for example `Ctrl-D`), and descriptions that wrap to the modal width
- [ ] #3 The viewer help groups its keys under section headings in the same style as the main help, and shows the same kind of footer saying how to scroll and close it
- [ ] #4 The viewer help scrolls when it does not fit a short terminal, the same way the main help does, and keys that close or scroll it match the main help where they do not clash with viewer keys
- [ ] #5 The viewer help rows come from, or are checked against, `VIEWER_KEYS`, so a key added to the viewer cannot be left out of its help
- [ ] #6 A test in `src/tui/consistency_tests/` asserts that the two help modals share the same frame and column styling, so they cannot drift apart again
- [ ] #7 Checked with `conspectus tui --snapshot` for both modals at a normal and a narrow width, and existing viewer and help snapshot tests are updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
