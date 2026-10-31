# 0005: A control that moves in unifies on the best variant and catalogs what users see change

| | |
| --- | --- |
| Status | Accepted |

## Decision

- When a control comes into the framework from its host, it takes the best
  variant of each behavior the host had, in the same change, instead of
  copying the host's disagreements into the library.
- Every change that alters what a user sees or does lists each difference in
  its summary under two headings: **Regressions** (something users had that
  is gone or worse) and **Consistency wins** (a difference that exists
  because the control now follows a house rule), with the rule's number
  where one applies. A change with none says so explicitly.
- The catalog is checked on rendered frames, at a normal and a narrow width,
  focused and unfocused, not by reading code. Tests and snapshots that encode
  the old behavior are updated deliberately and the summary says so.
- A change a user would be surprised by also goes in the host's user-facing
  documentation and changelog.

## Rationale

Extracting first and unifying afterward costs roughly twice the work, and
the disagreements are copied into the library only to be changed a second
time. Naming every visible change makes the deliberate ones reviewable and
exposes the accidental ones.

## Consequences

- Some visible changes are real and deliberate: lists that start wrapping,
  forms that take the theme and gain an error line, a colored border on the
  focused pane, cancel messages that disappear.
- The how-to is in
  [contributing.md](../contributing.md#cataloging-visible-changes).

## Alternatives considered

- **Behavior-preserving extraction first, unification afterward.** Each step
  a pure refactor, but twice the steps and the disagreements copied in.
- **Each surface keeps its own semantics.** Preserves muscle memory, but the
  rules stay unlearnable.

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 5 (the behavior change policy).
- host ADR 0151, "The framework control set and its house rules": the house
  rules' preamble (each extraction records every visible change as a
  regression or a consistency win), its consequences, and the alternatives.
