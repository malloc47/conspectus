---
id: DRAFT-001
title: Decide whether the TUI turns on mouse capture
status: Draft
assignee: []
created_date: '2026-11-13 20:41'
labels:
  - h-scroll
  - tui
  - tui-framework
milestone: m-11
dependencies: []
references:
  - src/tui/runtime/term.rs
  - tests/terminal_hygiene.rs
  - crates/tuikit/docs/controls/pager.md
  - crates/tuikit/docs/house-rules.md
priority: medium
type: spike
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI turns on raw mode, the alternate screen, and bracketed paste (`src/tui/runtime/term.rs`) and asks for no mouse mode. The wheel works only because of the layers in between, and differently in each:

- Directly under a terminal or mosh, the terminal's alternate-scroll mode turns each wheel event into cursor keys. mosh keeps the local terminal in the alternate screen and copies the app's mouse modes, so with none requested the terminal does this. Ghostty sends three keys per tick by default (`mouse-scroll-multiplier`), so a notch moves the selection two or three rows, and a fast spin queues dozens of keys (CSP-874).
- Inside tmux with `mouse on`, tmux 3.7c drops wheel events for a pane that hasn't asked for the mouse, so the wheel does nothing. This was verified 2026-11-13 against a private tmux server with the operator's config, including a pane with alternate-scroll mode (`?1007`) set.

With mouse capture on, the wheel would reach the TUI as wheel events. The TUI could pick its own step (one row per notch, or scroll the view without moving the cursor), merge a burst of reports into one move, and the wheel would work inside tmux too. Capture is also the precondition for CSP-344 (wheel and click in the viewer) and CSP-355 (expand on click).

Drawbacks to weigh:

- **Text selection.** The terminal's own drag-to-select and copy stop working over the TUI; the operator has to hold Shift (or Option in some macOS terminals). Inside tmux, a click or drag in the pane goes to the TUI rather than to tmux's copy mode. Touch selection in mobile clients such as Termius may change as well.
- **The multiplier stays.** Ghostty sends one wheel report per row of `mouse-scroll-multiplier`, and mosh delivers every report, so the TUI still has to merge reports or choose its own step. Capture changes who decides the step; it doesn't reduce the input.
- **Every control needs mouse behavior.** Each pane and control needs a defined wheel and click behavior. That is framework work in `tuikit`: routing `Event::Mouse` to the top modal, and the Tier 2 hit testing CSP-344's notes describe. The house rules and consistency tests would need rules for it, and a host-only mouse path would split the conventions.
- **Mode hygiene.** The mode has to be turned off on every hand-off of the terminal (attach, the external viewer), on exit, and on panic, and back on after a return. A mode left on makes the shell print escape sequences for every click. `tests/terminal_hygiene.rs` keeps `term.rs` the one place modes change.
- **Input traffic.** Any-motion tracking (`?1003`) sends a report for every mouse movement, which over mosh is a steady packet stream. Button-event modes avoid that but still send more bytes per notch than cursor keys do.
- **Configuration.** Operators who copy text from the TUI often may want capture off, which would add a setting and a second behavior to test.

The alternative is to leave capture off, as today, and rely on CSP-874 to absorb cursor-key bursts. The wheel then keeps the terminal's step and stays dead inside tmux.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records whether the TUI turns on mouse capture and, if so, which modes, what the wheel and a click do in each pane and modal, and how the operator selects text; it weighs the drawbacks listed in the description against leaving capture off
- [ ] #2 If the decision changes how `tuikit` controls take mouse input, a framework decision record names the ADR, and `docs/adr/README.md` marks the ADR framework-owned in the same change
- [ ] #3 CSP-344 and CSP-355 are updated to follow the decision, and any follow-up implementation is filed as tasks
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
