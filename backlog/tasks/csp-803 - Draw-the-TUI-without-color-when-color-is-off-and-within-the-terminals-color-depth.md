---
id: CSP-803
title: >-
  Draw the TUI without color when color is off, and within the terminal's color
  depth
status: To Do
assignee: []
created_date: '2026-10-31 08:06'
labels:
  - tui-framework
  - tui
dependencies: []
documentation:
  - docs/plans/tui-framework/gap-analysis.md
  - crates/tuikit/docs/roadmap.md
priority: medium
type: bug
ordinal: 889000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus tui --color` resolves `--color`, `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and `TERM=dumb` the way every command does (ADR 0022, `resolve_color_from_env` in `src/cli/mod.rs`) into `TuiOptions::color` (`src/tui/mod.rs`), and nothing reads that field: the TUI draws its theme's colors under `NO_COLOR=1` and `--color never`, although its `--help` says the flag controls when output is colorized. Nothing detects the terminal's color depth either, so a theme value written as RGB (`#ff8800`) or as a 256-color index (`ansi256:42`), both of which the `[tui.theme]` grammar accepts, shows as whatever a 16-color terminal makes of it.

The TUI framework's gap analysis ranks `NO_COLOR` and color-depth downgrade Tier 1, and the framework roadmap (`crates/tuikit/docs/roadmap.md`) lists the item open. Meaning already survives without color: the pointer mark shows the cursor row (house rule 1) and every status is a glyph plus a word. The framework reads no environment of its own, so the host detects and the framework applies; a theme is a plain value (`crates/tuikit/docs/controls/theme.md`). Theme presets are CSP-269; this story is the switch and the downgrade. Found by the backlog impact review of the TUI framework milestone (CSP-766).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With `NO_COLOR` set (under `--color auto`) or with `--color never`, the TUI draws no foreground or background color in any view, overlay, or the transcript viewer; modifiers stay, and the cursor row, the focused pane, and every status stay distinguishable (checked with `conspectus tui --snapshot` and a test over a drawn buffer)
- [ ] #2 `--color always` draws colors even with `NO_COLOR` set, and the default on a terminal is unchanged
- [ ] #3 On a terminal that reports 256 colors, RGB theme values draw as the nearest 256-color index; on one that reports 16 or fewer, RGB and indexed values draw as the nearest of the 16 ANSI colors; a truecolor terminal is unchanged; how the depth is read (`COLORTERM`, `TERM`) is documented
- [ ] #4 Conspectus reads the environment; removing color and downgrading it are `tuikit::theme` functions over a theme value with framework tests, and the framework independence guards pass
- [ ] #5 `docs/operations.md` describes the TUI's color behavior, and `crates/tuikit/docs/roadmap.md` marks the Tier 1 item done
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
