# 0010: Controls draw in immediate mode on ratatui, under no component framework

| | |
| --- | --- |
| Status | Accepted |

## Decision

- Every control draws into a ratatui `Buffer` on every frame from the state
  it is given. Nothing is retained between frames but values a control or
  its host keeps on purpose (a cursor, a scroll window, a layout keyed by
  width).
- No retained-mode or component-tree framework sits on top of ratatui: no
  mounted component tree, no view tree with callbacks, no reactive system.
  Controls are plain types with `handle` and `render`, and a host composes
  them in its own loop.

## Rationale

Immediate mode keeps drawing a function of state, which is what the frame
driver and the goldens rest on, and it never has to update retained widget
state from background work. Ratatui's buffer API draws without a terminal,
so every frame can be tested at a fixed size. A component framework's mount
and focus model adds layers without removing complexity for an interface of
a few panes and modals over one model, and would fight the frame tests.

## Consequences

- State a control needs across frames is an explicit value it keeps or
  returns (record [0019](0019-one-scroll-window-and-one-follow-rule.md)).
- Tests render controls into buffers at fixed sizes (record
  [0042](0042-key-scripts-and-a-frame-driver.md)).

## Alternatives considered

- **A component framework over ratatui** (`tui-realm`) or **a view-tree
  library with its own widgets** (`Cursive`). Their abstractions fight
  custom row composition and buffer tests.
- **Raw terminal libraries** (crossterm, termion, termwiz alone). Tables,
  trees, scrolling, and overlays from primitives is avoidable risk.
- **Caching view trees across frames**, or **a reactive rewrite**. Wrong
  regime at this scale, and they would undo the frame tests.

## Origin

- host ADR 0024, "The interface's runtime, architecture, and dependency
  policy": decision 1 (ratatui with crossterm, immediate mode, no framework
  on top) and its alternatives.
- host ADR 0085, "The interface's Elm/MVU architecture": its non-goals (no
  component framework, no retained-mode or reactive rewrite).
