# `conspectus tui` Sessions View UX Review

This review responds to the live sessions-view snapshot captured during
Phase 8 implementation. It is intentionally critical: the current view is
already operational, but the default sessions surface should become faster to
scan, more spatially efficient, and more trustworthy as a session switcher.

The benchmark use case is the Returning Operator from
[`docs/implementation/phase-08-interactive-tui.md`](implementation/phase-08-interactive-tui.md):
open the TUI, recognize the right session within a few seconds, and attach
without reconstructing state from long paths or raw provider identifiers.

## Overall Assessment

The two-pane shape is the right foundation. It gives the operator a stable
left-side navigation surface and a right-side confidence surface before
attaching. The biggest issue is not the concept; it is information hierarchy.
The live snapshot gives too much equal visual weight to raw identifiers,
full-ish paths, wrapped tmux names, and pane borders, while the key decisions
the operator is making are simpler:

- which project is this?
- which sessions are alive or attachable?
- what was each one doing?
- what will pressing `a` attach to?

At the moment, the UI answers those questions, but not cheaply. It requires
reading long rows, mentally decoding glyphs, and looking past clipped provider
strings. For fast session switching, the interface should be closer to a
radar: compact groups, strong selected state, clear status chips, and a
preview that looks like the terminal the user will enter.

## Left Pane: Tree And Density

### What Works

- The repo/workspace grouping matches the graph model and keeps sparse sessions
  from becoming an unstructured global list.
- Expand/collapse affordances are familiar and scale to large session counts.
- Inline previews are valuable; they are often a better recognition cue than a
  session id.
- Mux candidate rows are visible in-place, which is the right passive
  treatment for ambiguity.

### Problems

- **Group labels consume too much width.** Long paths and raw tmux ids wrap or
  truncate in ways that compete with actual session rows.
- **Rows do not yet read as columns.** The snapshot shows `id`, harness,
  placeholder dashes, and mux glyphs, but the alignment is weak enough that the
  eye has to parse each row individually.
- **Glyph semantics are not self-evident.** `◉`, `◐`, and `◯` are compact, but
  without color and occasional text reinforcement, new or tired users must
  remember the legend.
- **The selected row is under-emphasized.** A switching UI needs the current
  attach target to be unmistakable, especially when several rows have similar
  ids and harness labels.
- **The "whole world" goal needs a launch-context cue.** Showing all projects is
  right, but the current working repo should still be a lightweight anchor,
  preferably expanded and marked without hiding other groups.

### Recommendations

- Treat the left pane as a fixed-column list, even inside a tree:
  `disclosure/depth`, `short id`, `harness`, `age`, `status`, and optional
  preview. Keep those cells visually aligned across siblings.
- Use semantic row color sparingly:
  - selected row: inverse or high-contrast highlight across the full row
  - harness token: existing per-harness palette
  - mux status: green attached, yellow ambiguous, dim un-muxed
  - stale or failed discovery: dim row text plus a warning chip, not red rows
- Compress group labels:
  - show basename or `repo-name` as the primary label
  - keep shortened path as dim secondary text only when width allows
  - expose the full path in the right panel or help/status on demand
- Add a small launch-context marker to the current repo/checkout group, such as
  `● cwd`, `here`, or a subtle accent on the group row. Do not filter the world
  to the current directory.
- Add a density toggle after the core layout stabilizes:
  - compact: one line per session
  - comfortable: selected plus recent inline previews
  - expanded: preview under every visible session when space allows

## Right Pane: Detail And Preview

### What Works

- The detail panel gives the operator confidence before attaching.
- Header plus preview is the correct split; tabs would slow the primary path.
- Showing mux capture for muxed sessions is the right default because it
  reflects the terminal surface the user will enter.

### Problems

- **Raw mux identifiers dominate the detail pane.** The tmux id wraps over
  multiple lines and pushes useful preview content down. This is especially
  costly because the full native id is rarely the user's recognition cue.
- **Preview fidelity is too low without color.** A tmux preview displayed as
  plain monochrome text feels less trustworthy than the color TUI surrounding
  it, and it loses semantic highlights from the agent/tool session.
- **Detail fields mix identifiers and decisions.** The operator needs
  "attachable: yes", "target: preferred mux", "ambiguity: N candidates", and
  "cwd/project", while raw ids are secondary.
- **Long preview lines wrap awkwardly.** The pane should preserve enough
  terminal structure to be recognizable without becoming a hard-to-read text
  dump.

### Recommendations

- Render detail fields in priority order:
  1. action state: attachable / un-muxed / ambiguous / unavailable
  2. project/cwd and harness
  3. title or last meaningful session label
  4. PR/branch/fork context when present
  5. raw ids last, dimmed or hidden behind a copy action
- Replace raw native mux ids in the main field with a display name. Keep full
  native id available through `i`, `node show`, or a detail overflow line.
- Capture tmux previews with ANSI color when possible and render the result as
  styled spans in Ratatui. Fall back to plain text when `--color=never`, the
  capture lacks ANSI, or parsing fails.
- Add a compact preview header such as `preview · tmux · captured 2s ago` and
  use it to surface stale/error states.
- Consider cropping tmux previews to the bottom N lines by default. The bottom
  of an agent pane is usually the most useful recognition surface.

## Header And Status Bar

### What Works

- The global header communicates view, freshness, and counts.
- The status bar already names the primary actions and shows focus state.

### Problems

- **The header is doing global work but not local orientation.** Counts are
  useful, but the operator also needs to know whether the current project is
  represented and selected by default.
- **Focus has no practical effect yet.** `Tab` changes the status suffix, but
  the panes do not visibly or behaviorally change enough for users to trust it.
- **Action availability is not visible enough.** In a session switcher, the
  availability of `a attach` should be obvious before the keypress.

### Recommendations

- Make focused pane borders or titles visibly different. When the right pane is
  focused, `J/K`, PageUp/PageDown, and scroll hints should clearly apply there.
- Make the status bar contextual:
  - selected attachable row: `a attach tmux:<display>`
  - ambiguous row: `a attach preferred · m choose`
  - un-muxed row: `a unavailable · R resume later`
  - group row: no attach action, show expand/collapse
- Reserve the right status zone for provider health and freshness chips, not
  keybinding overflow.
- Add a help legend for mux glyphs and status colors, but keep it out of the
  normal status bar once users have learned the UI.

## Color And Symbols

Color should reduce reading, not decorate the interface. The best use of color
in this view is semantic:

- harness identity: stable per-harness color, reused from table output
- mux state: green/yellow/dim
- selected row: inverse/high-contrast
- focused pane: subtle accent
- provider failures: red chip only, not broad red content
- stale data: yellow or dim freshness chip

Symbols should stay few and learnable:

- `▶` / `▼`: disclosure
- `◉`: selected/preferred attach target exists
- `◐`: attach target exists but is ambiguous
- `◯`: no mux target
- `!` or `⚠`: provider or relationship warning, used sparingly

Avoid adding more symbolic columns until search, sorting, and view switching
are fully usable. Dense symbol clusters become harder to scan than short words.

## Fitness For Fast Session Switching

The current interface is close to fit-for-purpose but not yet fast. It can
attach, preview, and navigate, which are the hard functional pieces. The main
remaining UX work is lowering recognition cost.

For fast switching, the default frame should make the next action clear without
opening help:

- launch shows the whole world, with the current project expanded and selected
  as the starting context
- the selected row has a strong full-width highlight
- attachability is visible in both the row and status bar
- the right preview looks like the tmux pane the operator will enter
- raw ids never consume prime space unless they are the selected object
- search/filter can narrow a large world quickly

The operator should be able to use the TUI mostly by shape and color after the
first few sessions: project group, harness color, recency, mux state, preview.
If the user must read raw tmux names or long paths on every attach decision,
the UI is functioning but not yet optimized.

## Suggested Backlog Themes

The concrete follow-up tasks are tracked in `docs/backlog.md` under Phase 8:

- `T8-010`: render ANSI color in tmux previews
- `T8-011`: strengthen selected/focused visual states
- `T8-012`: compress group and mux display labels
- `T8-013`: make launch context a default expansion/selection hint
- `T8-014`: make the status bar contextual to the selected row
- `T8-015`: add a density mode for the sessions tree
- `T8-016`: crop and annotate tmux previews for recognition
- `T8-017`: add a visible search/filter workflow
