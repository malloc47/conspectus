# 0002: The framework holds components and conventions; the runtime stays with the host

| | |
| --- | --- |
| Status | Accepted |

## Decision

The framework covers two of an interface's three layers:

1. **Components and design tokens.** The theme, glyphs, text and scroll
   helpers, chips, ages, frames, the selection look, and the controls built
   on them.
2. **Interaction conventions.** Key tables that generate footers, hints, and
   help; the overlay contract and a modal stack the host owns; the focus
   rules; the feedback model; and a test kit that checks controls against
   the house rules.

The third layer, the application runtime, is each host's: the event loop,
the effect lifecycle and its executor, undo, and taking the terminal over
and handing it to a child process. A host runs its own loop and calls into
the controls; the framework never calls out to run anything.

The runtime comes into the framework only when a second program needs it:
one committed to run on the framework with an interactive loop of its own
that hands the terminal to a child process, runs background work that
reports back as messages, or wants a headless mode for tests. It comes then
as an opt-in layer that no control depends on (a sibling crate rather than a
module of this one), the terminal guard first: raw mode, the alternate
screen, bracketed paste, a restore that survives a panic, and suspending the
terminal around a child the host starts.

## Rationale

The runtime is the most entangled part of a program (its effects, its undo,
and its reducer know the program's own model), and the layers below are
useful without it. Leaving the loop to the host lets a program with another
loop (an asynchronous one, one fed by a server) use every control.

An evaluation measured the first host's runtime once the two layers had
moved: about a ninth of it is generic (the loop and its timing, the input
and paste rules, the terminal guard and the hand-off around a child, work
behind a spinner, the dispatch of effects as data, a headless driver); the
rest knows the host's own model. No second program existed, and a simple one
needs a loop of a few dozen lines, which the guide shows. Extracting the
generic part would have cost the boundary this record draws for no program
that uses it.

## Consequences

- The crate has no main loop, no terminal setup or teardown, and no effect
  type. The usage guide ([guide.md](../guide.md)) shows a host's loop.
- Extracting a runtime was evaluated and deferred until a second program
  meets the condition above; nothing in the crate assumes one.

## Alternatives considered

- **The runtime in the first cut.** Rejected: the most entangled part, and
  the layers below do not need it.
- **A framework that owns the loop** (a component tree that mounts and
  routes). Rejected in record [0010](0010-immediate-mode-and-no-component-framework.md).

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 2 (the two layers in scope and the runtime left out), and the
  alternative of including the runtime.
- host ADR 0200, "The application runtime stays with its host until a second
  program needs it": decisions 1 to 3 (the runtime stays out, the condition
  that reopens it, and the shape it would take).
