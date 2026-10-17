# ADR 0122: Harness-Hidden Records Behind A Viewer Toggle

## Status

Accepted. Extends ADR 0053.

## Context

ADR 0053 says the viewer's default render matches what the harness
showed the operator, with the on-disk reality behind a toggle. So far
that principle has one toggle, `I`, for aborted exchanges. Other
harness-hidden content has been dropped in the parser instead. The
Codex parser skips `developer` and `system` messages, and channel
markers are filtered out, so the operator can't see them in the viewer
at all.

Pi (ADR 0121) makes the gap concrete. A Pi session file holds:

- the `system` message with the prompt sections and tool declarations,
- extension `custom` entries (state the extension persisted for
  itself),
- `custom_message` entries, which Pi shows only when `display` is true,
- metadata entries: `model_change`, `thinking_level_change`, `label`,
  `usage`, `context_edit`.

Pi's UI shows none of these, but they explain what happened in a
session: which model answered, what an extension recorded, which
prompt the agent ran under. Dropping them in the parser hides them for
good. Showing them by default breaks ADR 0053.

Reusing `I` doesn't fit. "Aborted" means "an exchange the user
interrupted," and a system prompt isn't that.

## Decision

1. **`TranscriptTurn` gains a `hidden` flag**, set by a parser when the
   harness's own UI would not have shown the turn. It is
   harness-neutral, sits alongside `aborted`, and is serialized only
   when true.
2. **The viewer hides `hidden` turns by default.** `H` toggles them,
   the footer shows a `hidden·hide (H)` / `hidden·show (H)` chip, and
   the `?` panel lists the binding. The filter runs before the
   kind-based toggles, so a hidden tool call still needs `t` as well.
   The inline transcript preview uses the same filter.
3. **Parsers use the flag for content the harness hid but that is worth
   reading.** Pure noise, such as empty records or opaque blobs, can
   still be dropped. The Pi parser is the first user. Moving the Codex
   `system` and `developer` messages onto the flag is a follow-up, not
   part of this decision.

## Consequences

- Pi's system prompt, extension state, hidden extension messages, and
  model and usage changes are one keystroke away and invisible by
  default.
- Every parser that builds a `TranscriptTurn` sets `hidden: false`
  unless it has hidden content. Existing documents are unchanged.
- The footer gains a chip. In narrow terminals it drops out with the
  aborted chip, ahead of the tools and thinking chips.

## Alternatives Considered

### A. Drop Pi's hidden records, as the Codex parser does

Simplest, but the operator loses the model changes and extension state
that explain a session. Rejected in favor of the ADR 0053 pattern.

### B. Reuse the `I` toggle

Rejected. It would conflate two questions ("what did I interrupt?" and
"what did the harness keep from me?") under one label.

### C. One toggle per record type

Rejected. It would add footer chips and bindings for distinctions the
operator rarely needs. A single "show what the harness hid" switch
answers the question ADR 0053 asks.
