# 0066: The status bar names the keys of the scope that has them

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/status_bar.md](../controls/status_bar.md) |

## Decision

- **Two modes.** `StatusBar` shows either a message (a `Feedback` in its
  severity's style, cut with `…`) or the host's segments and the key hints
  on one priority line, in the cells the host's fixed spans leave: a lead
  in both modes, a scope shown with the keys only, and a trail.
- **Hints are generated.** `KeyHintBar` makes the hints from key tables and
  the scoped keymap, in their order: each shown row's label and verb in the
  footer's grammar; rows whose words match join their labels across the
  whole bar; a note is a hint with no key. A bound row carries an optional
  rank (`KeyRow::rank`); a row without one takes the rank the host gives its
  table. A footer's segments carry their keys and highest rank, so a
  control's footer goes into the bar whole.
- **Annotations word a row for where the user is.** Built from the host's
  state each frame, an annotation leaves a row out, gives it another verb, a
  detail after the verb (what it acts on), a reason it cannot act (drawn
  dimmed), or another rank. What a verb cannot carry is a field of the
  hint, not free text.
- **The scope that has the keys is the scope whose keys show.** With an
  overlay on top, its own hints (`Overlay::hints`); otherwise the focused
  pane's and the program's. An unavailable keymap binding is left out.
- Nothing writes key hints into the status line by hand.

## Rationale

House rule 8 for the status bar. Hand-written hints drifted from what the
keys did and were copied in when overlays opened. Generating the bar from
the tables that resolve the keys keeps it true, and showing the keys of
whatever has them tells the user what acts now.

## Consequences

- A host scans the code that writes its status line for hand-written hints,
  and the conformance suite checks that the bar names only keys the top
  modal takes (record [0044](0044-the-conformance-suite.md)).

## Alternatives considered

- **No hints under an overlay**, since the modal draws its own footer. An
  empty bar reads as nothing to do; the duplication is generated, so it
  cannot drift.
- **Naming the overlay in the scope tag.** It needs a word per modal that no
  table holds, and each modal's frame already names it.
- **Ranks from a closure over the action.** A rank is a property of a row's
  hint that its table can state once.

## Origin

- host ADR 0191, "The status bar names the keys of the scope that has them,
  generated from its key tables": decisions 1 to 5 and 7.
