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

## Rationale

The runtime is the most entangled part of a program (its effects, its undo,
and its reducer know the program's own model), and the layers below are
useful without it. Leaving the loop to the host lets a program with another
loop (an asynchronous one, one fed by a server) use every control.

## Consequences

- The crate has no main loop, no terminal setup or teardown, and no effect
  type. The usage guide ([guide.md](../guide.md)) shows a host's loop.
- Extracting a runtime later is a separate evaluation; nothing in the crate
  assumes one.

## Alternatives considered

- **The runtime in the first cut.** Rejected: the most entangled part, and
  the layers below do not need it.
- **A framework that owns the loop** (a component tree that mounts and
  routes). Rejected in record [0010](0010-immediate-mode-and-no-component-framework.md).

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 2 (the two layers in scope and the runtime left out), and the
  alternative of including the runtime.
