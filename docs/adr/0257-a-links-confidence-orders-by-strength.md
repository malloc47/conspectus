# ADR 0257: A Link's Confidence Orders By Strength

## Status

Accepted. States the direction of the tie-break that
[ADR 0035](0035-graph-to-view-query-layer.md) and
[ADR 0041](0041-resolver-stays-in-rust.md) describe as "provenance
precedence, then confidence, then link id". No new key, dependency, write
path, or change to the saved graph.

## Context

`Confidence` declares its variants `High`, `Medium`, `Low`, and derived
`Ord`, so `High` was the smallest value. Every place that picks the
preferred link ranks it with the idiom `right.confidence.cmp(&left.confidence)`,
the descending sort the same code uses for provenance precedence, match
evidence, and tier, where a bigger number is a stronger link. Applied to
`Confidence` it put `Low` first: among links that tied on every key ahead
of confidence, the weakest one won.

It surfaced while building CSP-908, when a test gave a second session's
link `Confidence::High` to make it the one `v` opens on a mux and the
`Medium` link kept winning. CSP-909 investigated. Nine places rank by
confidence, all meant to prefer the strongest link:

- `pick_preferred` (`src/model/link.rs`), behind the CLI output for agents,
  muxes, and forks;
- `GraphSnapshot::preferred_mux_for`, which decides a session's attach
  target and row glyph;
- `preferred_session_for_mux`, the session `v` opens on a mux;
- the resolver's generic, session↔mux, branch↔PR, and process-identity
  comparators;
- the explorer's link order, whose own comment calls its first link for a
  neighbor "its strongest candidate".

Nothing in the ADRs or comments says otherwise, and `H`/`M`/`L` in the
table output read as strongest first.

The inversion never changed an outcome. A probe on every comparator,
run over the whole test suite (6037 tests, the fixture corpus included),
logged no comparison that confidence decided. Producers keep confidence
in step with what decides first: a session↔mux link from an exact cwd is
`StrongDiscovered` and `High`, one from a cwd prefix is `Discovered` and
`Medium`, so the provenance or the evidence rank has already separated
them. It was latent: the first producer to tie on those keys with a
different confidence would have had its weakest link win, silently.

## Decision

1. **Among links that tie on the keys ahead of it, the more confident one
   is preferred.** `High` beats `Medium` beats `Low`. Confidence still comes
   after provenance precedence (or tier), match evidence, and the keys each
   relation's comparator ranks first, as before.
2. **`Confidence` orders by strength.** `Ord` is written out so that
   `High > Medium > Low`. The comparators are unchanged, since each already
   reads as "descending puts the strongest first", and one change corrects
   all nine places.
3. **The variants keep their form.** Their declaration order, their serde
   names, and their archived discriminants (`High` 0, `Medium` 1, `Low` 2)
   are as they were, so a saved graph and its JSON read the same. A test
   pins the discriminants.
4. **A test per place pins the direction.** Each ranks three links that
   tie on everything ahead of confidence, with ids that sort against their
   strength, so the id tie-break cannot stand in for it; others check that
   provenance and evidence still outrank confidence.

## Consequences

- No output changes today: no existing test or snapshot moved.
- A new producer, or a relation that ties on provenance, gets the order the
  documents implied.
- Any code that compares two `Confidence` values with `<` or `>` now means
  strength, which is how it reads.

## Alternatives Considered

- **Flip the operands at each of the nine places.** It fixes today's sites
  and leaves the trap: `right.x.cmp(&left.x)` is correct for every number
  beside it, so the next comparator would repeat the mistake.
- **Declare the variants `Low`, `Medium`, `High`.** It makes the derived
  order right, but the saved graph archives the variants by position, so it
  would change what an existing `graph.bin` means unless its format were
  versioned for the purpose.
- **A `strength()` method that every comparator calls.** It makes each call
  site explicit, at nine edits, and leaves `Ord` pointing the other way for
  the next reader.
- **Leave it.** Nothing breaks today, which is the reason it went unnoticed;
  the cost falls on whoever adds the first tying producer.

## Open Questions Answered

- **Was the derived order intended for some relation?** Nothing says so: the
  ADRs, the comments, and the display codes all treat `High` as strongest.
