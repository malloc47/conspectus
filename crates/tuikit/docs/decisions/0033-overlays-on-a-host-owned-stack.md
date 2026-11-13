# 0033: Overlays draw themselves on one modal stack the host owns

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/overlay.md](../controls/overlay.md) |

## Decision

- **One contract.** `Overlay<M, Cx>` has `handle(&mut self, key, &Cx) ->
  Outcome<M>` and `render(&mut self, area, buf, &Cx, &Theme)`, plus
  `fills_screen`, `takes_text` (a text field has the keys, so a paste goes
  there), and `hints` (the keys it takes now). `render` takes `&mut self`,
  so an overlay keeps the window it drew.
- **Outcomes.** `Consumed`, `Close`, `Dismiss`, `Commit(M)` (close, then the
  host handles `M`), `Back(M)` (record [0034](0034-steps-and-flows.md)), and
  `Stay(M)` (the overlay stays open and the host handles `M`). There is no
  "this key is not mine": an open overlay owns every key, which is the modal
  focus lock.
- **The stack is the host's.** `ModalStack<E>` is generic over the host's
  entry type (its own enum of overlays, or boxed trait objects). It routes a
  key to the top entry, settles the outcome (closing the top or its flow,
  and handing back what closed), draws every entry bottom to top from the
  topmost one that fills the screen, and opens and closes entries. The host
  owns it and lends it out with its own state as the context, so an entry
  reads live state and nothing is copied.
- **Leaving.** `Esc` steps back one level and closes at the first; `Ctrl-C`
  dismisses the whole overlay from any level; `q` dismisses too wherever
  nothing is typed. Closing is silent (house rules 4 and 5).

## Rationale

Modality in the host was enforced by the order of key wrappers and draw
calls alone: adding a modal touched a state field, a key wrapper, two event
loops, a draw call, and focus rules, none of which the compiler connected,
and only the top modal drew. One contract and one stack make adding an
overlay a state, an impl, and a registration.

## Consequences

- An overlay drawn under another stays drawn as it was.
- The conformance suite checks routing, the close keys, and drawing under a
  step (record [0044](0044-the-conformance-suite.md)).

## Alternatives considered

- **The stack beside the host's state** in a wrapper struct, so no lending
  is needed. The host opens overlays from many reducer arms, so the stack
  would travel with every update, or opening an overlay would become an
  effect.
- **A context narrower than the host's state** (a struct of borrowed
  fields). Some overlays read values the host computes in its own methods,
  which a view of borrowed fields cannot call.
- **Drawing only the top entry**, as before. An overlay under another would
  go undrawn, which any stacked flow trips over.
- **The stack owning `Ctrl-C` and `q`**, dismissing before the entry sees
  the key. A second source of keys that footers and the table checks cannot
  see; the keys stay in the tables.

## Origin

- host ADR 0085, "The interface's Elm/MVU architecture": contract 3 (a modal
  stack and one overlay contract).
- host ADR 0161, "Overlays draw themselves on one host-owned modal stack":
  decisions 1, 2, 4, 8, and 9.
- host ADR 0191, "The status bar names the keys of the scope that has them":
  decision 4 (an overlay names the keys it takes).
