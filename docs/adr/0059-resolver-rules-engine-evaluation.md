# ADR 0059: Resolver Rules-Engine Evaluation

## Status

Accepted

## Context

ADR 0041 (Resolver Stays in Rust) closed the question of porting the
resolver to SQL. A separate but related question keeps surfacing as
the resolver bug list grows
(`CSP-223/CSP-226/CSP-227/CSP-402/CSP-403`, the pin-binding pass,
`suppress_ambiguous_cwd_mux_links`, `demote_stale_source_mux_candidates`):
**is the hand-written resolver effectively recreating a rules
engine, and should it be reimplemented on top of one?** The nagging
shape is `CSP-096`'s `--explain` story — building a
proof-tree / justification surface from scratch sounds wasteful if
an off-the-shelf engine already provides it.

This ADR records the evaluation so the question stops re-surfacing
without an explicit re-trigger.

### What the resolver actually does

`src/resolve/mod.rs::resolve_snapshot` is five stages over a
`Vec<GraphLink>`:

| Stage | Code | Shape |
|---|---|---|
| A. Forward derivation of `LinkedToMux` from `MuxContainsProcess` ∧ `Process{Identifies,Candidates}Session` | `derive_process_mux_links` (`resolve/mod.rs:164-293`) | Datalog rule |
| B. Pin binding: synthesize `LinkedToMux` per pin, harness-filter, sort, pick top, emit diagnostics | `resolve/pins.rs:43-182` | Lookup + sort + pick-1 + fact synthesis |
| C. Demotion pre-pass: for each `(mux, LinkedToMux)`, mark older candidates `Overridden` when a fresher candidate sits within the activity window | `demote_stale_source_mux_candidates` (`resolve/mod.rs:64-144`) | Windowed `argmax` aggregation |
| D. Bucket-and-rank: group `(source, relation, target_key)`, sort each bucket by a per-relation lex-tuple comparator, pick `links[0]`, emit `Conflict` diagnostic for runners-up | `resolve_links` (`resolve/mod.rs:430-498`) | `argmax` over lex score tuple, per group |
| E. Post-filter: drop `LinkedToMux` winners whose evidence is `exact_cwd_match`/`cwd_prefix_match` if >1 logical session has any candidate on that mux | `suppress_ambiguous_cwd_mux_links` (`resolve/mod.rs:500-552`) | Negated existence filter |

Per-relation comparators (`compare_session_mux`, `compare_branch_pr`,
`compare_process_identity_links`) are *score functions* — fixed-width
lex-tuple comparisons. The evidence vocabulary is currently ~30
`match_kind` string tokens mapped to `u8` ranks by hand-coded
`match` arms (`mux_evidence_rank`, `process_identity_evidence_rank`,
PR analogues).

### Diagnosis

Stages A, C, and E are unambiguously rules-engine-shaped: forward
chaining, windowed aggregation, and negated existence respectively.
Stage A's bug surface (`CSP-223/CSP-227/CSP-402`) cluster on
join-key plumbing the engine would express directly. Stage C's
freshness tuning (`CSP-403`) is a one-line change in a
declarative form.

Stage D — the core ranking — is **not** rules-engine-shaped. RETE
conflict resolution selects which rule fires next; lex-tuple
`argmax` over candidate facts is a different problem. Both
production-rule and Datalog engines handle it via aggregation
extensions or scalar-score reductions, neither of which is more
natural than the current 5-axis Rust comparator.

Stage B straddles: the lookup/sort/pick-1 is imperative; the
synthesis of a new candidate is rules-shaped.

### Separating two questions

A subtle but load-bearing point surfaced during evaluation: the
"should we adopt a rules engine?" question and the "should we ship
an explainer?" question are independent.

- A rules engine gives you the *materials* for explanation (named
  rules, reified derivations, conflict-set traces). It does not
  give you a *comparator-level* explanation — "candidate X beat
  candidate Y at tie-break axis 3 because epoch 1717…2 > 1717…1"
  is not natively in any engine's output. That part is hand-built
  regardless of paradigm.
- Conversely, an explainer can be built on the existing typed
  scoring without adopting any engine, by promoting `MuxScore` /
  `PrScore` / `ProcessIdentityScore` to first-class data carried
  alongside `ResolvedRelationship`.

The bulk of `--explain`'s value (`CSP-096`) is comparator-level
on stage D, which is exactly the part no engine helps with.

### Library landscape

The credible Rust options surveyed:

**Ascent** — Datalog DSL via proc macro. Stratified negation,
aggregation (`agg`, `argmax`), lattice values, parallel evaluation.
Compiles rules to Rust at build time; no runtime dependency.
Academic origins, multi-author, paper-backed.

- Stages A, C, E translate directly to ~5-10 line rules each.
- Stage D expressible via `agg argmax` calling existing Rust
  comparators — gains marginal over current code.
- Fact ↔ model boundary is direct: Datalog relations are typed
  Rust tuples that round-trip cleanly with `GraphLink`.
- `RelationKind` exhaustiveness and `match_kind`-vocabulary checks
  remain compile-time errors.
- No proof-tree export. Explanation built by emitting an
  `explanation(WinnerId, Axis, Loser, …)` relation, same shape as
  hand-rolling it.

**rust-rule-engine (KSD-CO)** — RETE-UL + backward chaining,
GRL string DSL. v1.20.3, ~49 stars, single primary maintainer, MIT.
Notable: ships a first-class proof-tree exporter (`v1.9.0-alpha`,
matured through `v1.20.x`) with JSON / Markdown / HTML / console
output, plus a Proof Graph Cache (`v1.17.0`) and Truth Maintenance
System.

- Stages A, C, E expressible as GRL rules. Proof-tree export
  lands "for free" for the *derivation* axis.
- Stage D fights the engine: RETE + salience does not express a
  5-axis lex comparator cleanly. Either N rules per tier
  (`salience` math gets brittle, rule-count explodes) or a scalar
  score computed in a `then` block (puts the comparator right
  back in imperative code).
- The proof tree explains derivation, not comparison. The
  comparator-level questions CSP-096 actually needs to answer
  still require typed score breakdowns alongside, same as today.
- **Authoring is GRL strings only.** The API surface is
  `load_rules_from_file`, `load_rules_from_string`, and
  `add_rule(rule: Rule)` — but no public constructor or builder
  for `Rule` is documented. There is no closure API, no derive
  macro, no trait you `impl` on `GraphLink`.
- **Facts are string-keyed property bags in both `Facts` and
  `TypedFacts` modes.** `TypedFacts` refers to typed *values*
  (`FactValue` enum), not user types. Every `GraphLink` would
  flatten to `link.id.source`, `link.id.match_kind`, …
  per snapshot, then reverse-project from rule output.
- Cost paid on **every resolve** (projection both ways, lost
  enum exhaustiveness); proof-tree benefit paid only when
  someone runs `--explain`.
- Cuts directly against `CLAUDE.md` "design data-model-first" and
  ADR 0016 (dependency thinness for distribution).

**Cozo** — embedded Datalog DB. Same logical fit as Ascent for
derivation; coupled to a third storage layer alongside SQLite
(ADR 0037) + Rust model. Out of scope without a separate
storage-layer decision.

**Crepe** — Datalog macro, no aggregation. Strictly weaker than
Ascent for this workload.

**Datafrog** — low-level runtime (used by Polonius). Wrong
abstraction level for a CLI utility.

**scryer-prolog / clingo-rs** — only paradigms with native proof
trees + native preference semantics, but embedding cost
(distribution, mental model, FFI in clingo's case) is prohibitive
under ADR 0016.

**Custom defeasible-logic mini-engine** — academically the
cleanest fit (declared *defeats* discovered = attack relation).
Requires maintaining a 500-LOC in-house reasoner; ADR 0041's
posture explicitly disfavors taking on rule-engine infrastructure
without a concrete forcing function.

### Bug-class evidence

Of the ten most recent resolver-adjacent stories
(`CSP-358/CSP-359/CSP-249/CSP-357/CSP-402/CSP-403`, `CSP-364`,
`CSP-399`, `CSP-096`):

- Stage A/C/E derivation-class: `CSP-249/CSP-402/CSP-403`,
  `CSP-364`. Four of nine closed/active.
- Stage D comparator-class: zero closed bugs but `CSP-096` is
  driven entirely by stage-D opacity.
- Discovery / sidecar / schema: the remaining five, unaffected
  by resolver shape.

The derivation-class is real and growing, but not yet at a rate
that overcomes the impedance-mismatch costs evaluated above.

## Decision

1. **Build the `--explain` surface in Rust against the existing
   architecture, not on top of a rules engine.** Promote
   `MuxScore`, `PrScore`, `ProcessIdentityScore` to public typed
   values and carry them on `ResolvedRelationship` as a
   `score_breakdown` field. Have each per-relation comparator
   return both the `Ordering` and the breaking axis as data.
   This is `CSP-096`'s scope and closes the comparator-level
   explanation gap no rules engine resolves anyway.

2. **Do not adopt rust-rule-engine.** The typing erosion paid on
   every resolve (string-keyed property-bag projection, lost
   `RelationKind` / `match_kind` exhaustiveness) is the wrong
   tradeoff for a code path whose recent bug class is precisely
   field-name / join-key mistakes in imperative derivation
   passes. The proof-tree export is a real asset but addresses
   the derivation axis only, and the comparator axis (what
   `--explain` actually needs) is hand-built anyway.

3. **Do not adopt Ascent yet, but keep it as the named
   contender** if derivation-pass bugs continue to accumulate
   after `CSP-096` lands. Ascent's compile-time codegen,
   typed-tuple facts, and preservation of enum exhaustiveness
   align with the project's data-model-first posture; its lack of
   built-in proof trees is not a regression relative to what we'd
   build for `--explain` anyway.

4. **Set an explicit re-trigger** rather than leaving this open:
   if three or more derivation-pass bugs land after `CSP-096`
   ships, reopen this ADR and evaluate Ascent for stages
   A / C / E specifically (keeping stage D in Rust). Aesthetic
   preference is not a trigger; a counted bug class is.

## Consequences

- `CSP-096` proceeds as scoped: typed score-breakdown carrier,
  comparator returns axis-as-data, `--explain` renders the
  breakdown for both JSON and TUI detail. No engine dependency.
- The resolver code stays single-language and single-paradigm.
  Contributors do not need to learn a DSL or property-bag
  projection to work on it.
- The match-kind vocabulary remains hand-coded match arms in
  `mux_evidence_rank` / `process_identity_evidence_rank` /
  `pr_state_rank`. Adding a new token still requires touching the
  rank table; the compiler catches missing arms via warnings on
  exhaustive matches against `Option<&str>` patterns (but does
  not enforce closure over the open string set — that's a
  separate refactor, out of scope here).
- The derivation passes (A, C, E) remain imperative and remain
  the bug-attractor surface they are today. The re-trigger
  clause in (4) keeps this honest rather than asserting "Rust is
  always fine."
- Ascent and rust-rule-engine are named explicitly so future
  re-evaluation starts from this comparison rather than rederiving
  the landscape.

## Alternatives Considered

- **Adopt rust-rule-engine for stages A/C/E only, keep stage D
  in Rust.** Rejected for the typing-erosion reason in §Decision
  (2): the property-bag projection cost is paid on every resolve,
  while the proof-tree benefit only pays out on `--explain`
  invocations. Net negative even on the in-scope stages.
- **Adopt Ascent for stages A/C/E only, keep stage D in Rust.**
  Deferred. The shape is correct (typed tuples, compile-time
  codegen, no distribution cost) and the bug-class evidence is
  trending in this direction, but ADR 0041 just locked in
  "resolver stays in Rust" and overturning it within months
  without a hard forcing function is premature. The re-trigger
  in §Decision (4) handles this concretely.
- **Adopt Cozo and absorb the resolver + storage layer in one
  step.** Out of scope without a separate storage-layer
  decision; coupling them here would conflate two independent
  questions.
- **Build a custom defeasible-logic mini-engine.** Academically
  appealing (the `Overridden` state is literally an attack
  relation), but takes on rule-engine infrastructure ownership
  without a concrete pain point — same posture ADR 0041 closed
  with for the SQL question. Skip.
- **Adopt scryer-prolog or clingo-rs for native proof trees +
  native preference semantics.** Distribution cost prohibitive
  under ADR 0016. Skip.
- **Do nothing on `--explain` and revisit after more bugs.**
  Rejected because the comparator opacity is itself a *cause* of
  bug-investigation cost — without `--explain`, every stage-D
  surprise requires reading code to reconstruct the score axes.
  Building the explainer is independent of the engine question
  and pays off whether or not (3) is later reversed.

## Open Questions Answered

- **Is the matching logic effectively a rules engine?** Partially.
  Derivation stages (A, C, E) are rules-engine-shaped; the
  ranking core (D) is not. Adopting an engine would express the
  former cleanly while fighting the latter.
- **Does any Rust engine give a proof tree for free?**
  rust-rule-engine does, but only for the derivation axis. The
  comparator axis (the part `CSP-096` cares about) is hand-built
  regardless of engine choice.
- **Is this decision reversible?** Yes. The re-trigger in
  §Decision (4) names a concrete condition for reopening.
- **Does this affect ADR 0041?** No. ADR 0041 closed the
  "resolver in SQL" question; this ADR closes the "resolver in a
  rules engine" question. Both arrive at "stay in Rust for now"
  via different evidence.
