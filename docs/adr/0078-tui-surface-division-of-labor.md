# ADR 0078: TUI Surface Division Of Labor

## Status

Accepted

## Context

CSP-418 audited the sessions-pane top header element-by-element and
found that several pieces were paying for signals that already lived
in stronger places elsewhere on the screen: the `Conspectus` brand
was self-evident inside the running TUI, the `sessions` view label
duplicated the highlighted active view on the left-panel title strip,
the three-bucket mux chip block (`◉ N · ◐ N · ◯ N`) duplicated
per-row signals that ADR 0072 already made binary at the row level,
and the per-harness chips duplicated the row badges + group summary
chips. The audit recorded those findings in
`docs/plans/sessions-header-audit.md` and the implementation landed
the deletions.

The recurring shape of those decisions was "this signal already lives
somewhere stronger." Future header / status-bar / row-tree work will
ask the same question — does the new element earn its cells against
the signal it would carry? — and without a written rubric the next
audit re-derives the answer from scratch. ADR 0067 captured the
snapshot rule; ADR 0072 captured the per-row mux glyph contract; this
ADR captures the higher-level division of labor across the three
chrome surfaces.

## Decision

Conspectus's TUI chrome divides into three load-bearing surfaces, each
with a distinct contract about what it carries. Future visual work on
any chrome surface respects the contract; cross-surface duplication is
the audit signal to delete.

### 1. Top header (first row)

Carries:

- **Snapshot freshness** (`updated Ns ago · `). Primary content
  signal — the operator reads it whenever they suspect staleness,
  typically after `r` refresh or a background discovery tick.
- **Load-bearing counts** (`N/M sessions · M mux`). The visible-vs-
  total signal that's only available by aggregating the row tree;
  no individual row carries it.
- **Opt-in aggregates** when the operator sets a config knob.
  `[tui] show_harness_chips = true` is the v1 example: when set, the
  per-harness pills append after the prefix. Future opt-in chips
  (per-PR-state counts, per-recency-bucket counts) follow the same
  shape — explicit operator opt-in, off by default, the header stays
  short for the default operator.
- **Actionable triage chips** when the underlying state demands
  attention. `⚠ N` (ambiguous session count) renders only when
  N > 0 — when there's nothing to triage, the chip absents itself so
  the header's shape carries information ("nothing to fix").

Does **not** carry:

- The product brand. Self-evident inside the running TUI.
- The active view label. Already in the left-panel title strip below.
- Per-row signals (harness identity, mux state, recency bucket).
  Those live on the rows themselves.
- Aggregate counts that an operator can read off the rows
  (e.g., `◉ N` mux-attached count when the operator can scan the
  row tree's `◉` glyph per row).

### 2. Row tree (left panel body)

Carries:

- **Per-row identity** — the harness badge, the kind glyph (ADR 0073),
  the session id / mux name.
- **Per-row state** — the binary mux chip (ADR 0072), the recency
  bucket coloring, the bound-pin marker.
- **Per-group aggregates** — the group summary count chip `(N)` and
  the per-group ambiguity glyph `⚠`. These are aggregates the
  operator wants in scan position with their containing row.

Does **not** carry:

- Snapshot freshness. Read off the header.
- Contextual key hints. Read off the status bar.

### 3. Status bar (bottom row)

Carries:

- **Focus scope** (`[left]` / `[right]`) — which pane is active.
- **View-state chips** (`group:graph · filter:all · sort:hierarchy`,
  ADR 0031). Always-on signal of the operator's current view state.
- **Contextual key hints** that change with focus + selection. The
  status bar is the only chrome surface that varies with operator
  position; that's its load-bearing role.
- **Transient toasts** (CSP-326 / CSP-427). The toast surface
  overlays the bottom edge while a feedback message is live, then
  retires.

Does **not** carry:

- Primary content signals like freshness or counts. Those live in
  the header so they're visible regardless of focus or selection.
- Per-row signals. Those live on the rows.

## Consequences

**For the audit just completed:**

The deletions follow directly from the rubric: brand violated
"header doesn't carry self-evident identity"; view label violated
"header doesn't carry signals stronger elsewhere"; `◉ / ◯` chips
violated "header doesn't carry signals derivable from the row tree";
per-harness chips violated the same rule + dominated header width.
Freshness stayed in the header (primary content signal). View tab
strip stayed on the left-panel title (not on a content row).

**For future work:**

Any new chrome element gets the same question — "what does this earn
its cells back from?" — answered against the three contracts. The
answer is the audit's first gate; LOC / aesthetic considerations
come after.

Specific implications:

- The `[tui] show_harness_chips` opt-in is the template for any
  future opt-in aggregate. Adding a new opt-in to a contract surface
  is fine; making it default-on requires re-evaluating against the
  contract.
- The `⚠ N` ambiguity chip is the template for any future
  triage-only chip: render only when the underlying state has
  something to triage. Avoid the "always-on counter that's usually
  zero" pattern — it's the operator-time tax for nothing.
- The mux view's header is currently `N/M sessions · M mux` even
  though the view's primary kind is mux. That's a known wart per
  the audit; a future story may add per-view count language via a
  `header_primary_kind_word(view)` helper. Recording it as an
  open question rather than fixing it preemptively kept the audit
  PR scoped.
- The right-panel title strip — already styled to mirror the left's
  view-tab strip aesthetic — falls under the same "panel title
  carries identity + view affordances" rule. Changes there should
  consult this ADR.

**For status-bar evolution:**

The bottom-row contract leaves room for future additions (e.g., the
CSP-423 last-active-view indicator would naturally fit in the
view-state chip section). Adding chip-style status entries that
change with focus is on-charter; adding always-on primary signals
that don't change with focus violates the contract and belongs in
the header instead.

## Alternatives Considered

**A separate dedicated "info bar" row between the header and the
left-panel title.** Rejected during the audit: it would add a row
of overhead for signals the existing three surfaces already carry
or could carry. The header had room post-audit, the row tree's
border row carries the view tab strip, and the status bar carries
contextual hints — three rows is already the cap.

**Per-view header composition (different elements in mux vs sessions
vs PRs).** Rejected for this round: shared composition is simpler
to maintain; per-view divergence is churn for a marginal
operator-time win. The `N/M sessions` wording is uniform across
views in v1 (a known wart in the mux view); a future story can
revisit if operators report friction.

**Move freshness to the status bar.** Rejected because the status
bar already carries contextual hints that change with focus, so
freshness would compete with load-bearing transient text. Freshness
is a primary content signal and belongs in the header.

**Drop the view tab strip from the left-panel title (the strip's
`▸ sessions · mux · union · prs · forks` line).** Considered briefly
as part of the audit's view-tab-strip open question. Rejected because
the strip lives on a border row — not a content row — so its real
estate cost is zero. The strip is the operator's view switcher
affordance; removing it would push that signal back into the
top header (already removed during the audit) or into a separate
chrome surface (worse).

## Open Questions Answered

The audit's seven decision points all resolved per the recommendations
in `docs/plans/sessions-header-audit.md`. Recording them here as the
canonical reference:

1. Brand + view label dropped from header prefix.
2. `N/M sessions` wording (vs `N of M agents`).
3. Per-harness chips opt-in via `[tui] show_harness_chips`.
4. Mux state chips collapsed to `⚠ N` rendered only when N > 0.
5. Freshness stays in header (primary content signal).
6. Applied uniformly across all views (shared composition).
7. This ADR memorializes the cross-surface rubric.

## Open Questions Deferred

- **Per-view count language.** `N/M sessions` is uniform; mux /
  union / prs / forks views might benefit from per-view words via
  `header_primary_kind_word(view)`. Defer until operator feedback or
  a concrete story demands it.
- **Header layout for mobile-narrow terminals (< 40 cols).** The
  post-audit header fits at 50 cols, but very-narrow terminals
  (e.g., embedded systems, side-by-side splits) might still
  overflow. Defer until the use case surfaces; the truncation
  behavior is acceptable in the meantime.
- **Status-bar evolution under CSP-423** (persist last-active view).
  The view-state chip section is the natural home for a last-active
  marker; ADR 0078 anticipates this as on-charter for the status bar.

## Related ADRs

- ADR 0022 (PR-state coloring) — palette baseline used by the
  status-bar chip colors.
- ADR 0031 (filter / view switching / per-view state) — the source
  of the status-bar view-state chips.
- ADR 0032 (TUI palette) — the `[tui.theme]` flat-key model the new
  `[tui] show_harness_chips` knob mirrors.
- ADR 0067 (snapshot fixture mode) — the regression-net rule the
  audit's tests follow.
- ADR 0072 (mux indicator attachable-binary) — the per-row chip
  contract that made the three-bucket header section redundant.
- ADR 0073 (node-kind visual identity) — the per-row glyph language
  that lives on rows, not in the header.
