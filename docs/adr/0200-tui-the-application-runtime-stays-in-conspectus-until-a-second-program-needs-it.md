# ADR 0200: The Application Runtime Stays In Conspectus Until A Second Program Needs It

## Status

Accepted. Settles the evaluation ADR 0150 decision 2 deferred: the
application runtime (layer 3) stays out of the framework, with a trigger for
revisiting it. Framework-owned in part: the framework's record 0002 restates
the boundary and the trigger; the follow-up that folds the keymap's `Action`
layer into `Msg` is Conspectus's.

## Context

ADR 0150 put layers 1 and 2 (components and design tokens, interaction
conventions) in `tuikit` and left layer 3 in Conspectus: the MVU loop, the
effect lifecycle, undo, and suspending the terminal for a child process. It
carried one optional story to evaluate extracting the runtime once the layers
below had moved. They have, and the evaluation
(`docs/plans/tui-framework/runtime-evaluation.md`) measured the runtime line
by line:

- The runtime proper (`src/tui/runtime.rs` and `runtime/`, `effect.rs`,
  `undo.rs`, `snapshot.rs`, `keymap.rs`) is 5,899 lines: 643 generic, 4,968
  domain, 288 wiring. The generic share is 11.5%; counting the reducer and
  the `App` state, 6.2%.
- The generic part is the loop and its timing (70), input and drawing (82),
  the press filter (21), the terminal guard (51) and the hand-off frames
  around child processes (59), work behind a spinner (50), the effects-as-data
  dispatch skeleton (105), modal key routing (31), a bounded undo stack (29),
  and the headless snapshot driver (145).
- The domain part is the discovery workers and daemon IPC, the effects and
  their executors, launches and attaches, preview loading, fixture modes, and
  three copies of the key-to-handler dispatch over the keymap's `Action` (257
  lines), which ADR 0085 contract 1 meant to fold into `Msg`.
- Nothing else runs on the framework. The widget gallery
  (`examples/pantry.rs`) runs on `tui-pantry`'s loop, and the one planned
  candidate, a standalone transcript viewer binary (CSP-342), is unscheduled
  and would need a loop of about 30 lines.
- ratatui 0.30's `init`, `restore`, and `run` already restore raw mode and the
  alternate screen on a panic; Conspectus adds bracketed paste and the clear
  and wait rituals around a child.

An extraction is about nine stories, three of them large. It would change the
framework's founding boundary (its record 0002 and the "never" list of its
architecture guide: no loop, no terminal setup or teardown, no process-wide
state) and a guard, `tests/terminal_hygiene.rs`, which forbids taking the
terminal anywhere in the framework. Its riskiest paths, the hand-offs and the
worker threads, are covered only by the PTY end-to-end tests.

## Decision

1. **The runtime stays in Conspectus.** The framework gains no event loop, no
   effect type, no executor, and no terminal guard now. Its controls keep
   answering keys with outcomes and drawing into frames the host gives them.

2. **The trigger.** Extraction becomes worth doing when a second program is
   committed (a scheduled story, not an idea) to run on `tuikit` with an
   interactive loop of its own, and it needs at least one of: handing the
   terminal to a child process, background work that reports back as
   messages, or a headless mode for tests or snapshots. The story that
   schedules such a program reopens this decision.

3. **The shape when it comes.** The runtime is opt-in, never something a
   control depends on: a sibling crate beside `tuikit` (preferred), or a
   cargo feature, so a host with another loop still uses every control. It
   comes in this order: the terminal guard first (raw mode, the alternate
   screen, bracketed paste, a panic-safe restore, suspend and resume around a
   host-run child); then a `Program` and `Executor` seam whose loop owns
   input, ticks, resizes, and drawing while the host keeps its modal stack,
   its reducer, and its executors (which may borrow its model); then a
   headless driver over the test kit's `FrameDriver`. A program that needs
   only terminal safety takes the guard alone. The evaluation's outline is
   the starting plan.

4. **Host work first, and on its own merits.** Folding the keymap's `Action`
   layer into `Msg` and `Effect` (ADR 0085 contract 1), so the live loop, the
   fixture loop, and `--snapshot` route a key one way and differ only in
   their executors, is filed as CSP-802. It pays without any extraction and
   is the precondition for one.

## Consequences

- The framework's record 0002 stays and names this ADR with the trigger; its
  architecture guide's "never" list and `tests/terminal_hygiene.rs` stand.
- The framework's guide keeps showing a host's own loop around `AppFrame`,
  the modal stack, and `Toasts::tick`.
- `src/tui/runtime/term.rs` stays the one place that takes and returns the
  terminal.
- When CSP-342 or any other second program is scheduled, its story checks
  the trigger and, if it holds, files the extraction from the evaluation's
  outline.

## Alternatives Considered

- **Go now.** Nine stories, a changed boundary, and a changed guard, with no
  program to use the result: cost without a benefit yet.
- **No-go for good.** Closes the door although a second program that hands
  the terminal to children or runs background work would reuse most of the
  643 generic lines, including the parts that are hard to get right (paste
  mode on a panic, the clears around a child, drawing before each key).
- **Extract only the terminal guard now.** The smallest piece, and it would
  pay first, but it still breaks the "the framework takes no terminal" rule
  for no consumer, and ratatui's own `run` covers most of the safety.
- **Adopt a third-party application framework.** `tui-realm` and the
  component-tree frameworks were rejected by ADRs 0024 and 0085; nothing
  about the runtime changes that.

## Open Questions Answered

- *How much of the runtime is generic?* 643 of 5,611 classified lines
  (11.5%); 6.2% with the reducer.
- *Is there a second consumer?* No.
- *If the runtime moves, where to?* An opt-in layer beside the controls,
  terminal guard first.
