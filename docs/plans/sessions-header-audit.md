# Sessions-pane header audit (H-UI-004)

Status: draft, pending operator review before implementation.

## Why this audit exists

H-UI-001 (per-row binary mux chip per ADR 0072) and H-UI-002 (kind-glyph
identity per ADR 0073) reshuffled where ambiguity, harness identity,
and row-kind signals live in the TUI. The header was designed before
those shifts, and several of its elements were paying for signals that
moved elsewhere. This is the holistic re-evaluation: each element
gets the same test — "what does this earn its cells back from?" — and
the answers drive a small set of relocations, removals, and the
explicit decision to keep some of what's there.

## Today's header surface

Reading top-to-bottom from the rendered frame at 160 cols
(`cargo run --features snapshot -- tui --snapshot --snapshot-pane header`):

```
Conspectus · sessions · updated 0s ago · 122 of 118 agents · 11 mux  ·  [claude] 50  [codex] 46  [opencode] 26  ·  ◉ 10  ◐ 0  ◯ 112
```

That single bold prefix line carries five distinct elements joined by
` · ` separators, plus two appended chip sections that gracefully drop
in width-budget order. Decomposed:

| # | Element | Implementation | Width at default |
|---|---|---|---|
| 1 | `Conspectus` brand | hardcoded literal | 10 + 3 = 13 cells |
| 2 | `sessions` view label | `view_label(default_view)` | 8 + 3 = 11 cells |
| 3 | `updated 0s ago` freshness | `header_freshness(app)` | ~14 + 3 = 17 cells when present |
| 4 | `122 of 118 agents` count | `format_count_with_filtered` | ~17 + 3 = 20 cells |
| 5 | `11 mux` count | `snapshot_counts` | 6 cells |
| 6 | Per-harness chips | `append_header_chips` | up to ~40 cells |
| 7 | Mux-state chips | same function | 15 cells fixed |

Bare prefix (1–5): ~65 cells. Both chip sections add ~55 cells when
they fit; at <120 cells of header budget the harness chips drop, at
<70 the mux chips drop, leaving just the bare prefix.

Width snapshots (captured 2026-06-20):

**160 cols** — everything fits:
```
Conspectus · sessions · updated 0s ago · 122 of 118 agents · 11 mux  ·  [claude] 50  [codex] 46  [opencode] 26  ·  ◉ 10  ◐ 0  ◯ 112
```

**100 cols** — harness chips drop:
```
Conspectus · sessions · updated 0s ago · 122 of 118 agents · 11 mux  ·  ◉ 10  ◐ 0  ◯ 112
```

**70 cols** — chips entirely gone:
```
Conspectus · sessions · updated 0s ago · 122 of 118 agents · 11 mux
```

Separately, the left-panel title (rendered on the bordered Block above
the row tree) shows the **view tab strip**:

```
▸ sessions · mux · union · prs · forks
```

…with the active view highlighted in `panel_focus_accent` + BOLD and
the others in `secondary_text`. The strip is on the box border row, so
it doesn't compete with body content for height.

The status bar (bottom row) shows
`[left]  group:graph · filter:all · sort:hierarchy · <contextual key hints>` —
focus scope chip, ADR 0031 view-state chips, and contextual help.

## Per-element verdict

Applying the same test ("what does this earn its cells back from?")
to each element:

### `Conspectus` brand — **drop**

Once the operator is inside the TUI the brand is self-evident. It
takes 13 cells (10 + ` · `) and pays for nothing the operator
doesn't already know. The brand has its place on the welcome screen,
not on the dashboard header.

### `sessions` view label — **drop**

Duplicates the left-panel title strip's highlighted active view.
That strip ALREADY conveys "you're in the sessions view" with stronger
visual weight (BOLD + panel_focus_accent vs the bold-only header word)
and lower opportunity cost (it shares the border row, not a content
row). Dropping the header word reclaims 11 cells and removes a
two-place signal.

### `updated Ns ago` freshness — **keep, promote to lead**

This is the primary "is what I'm looking at fresh?" affordance. The
operator reads it whenever they suspect the snapshot is stale,
typically after `r` refresh or after a background discovery tick.
Moving it to the status bar (the story's open question) would lose
primary visibility — the status bar already carries contextual hints
that change with focus + selection, so freshness would compete with
load-bearing transient text. Keep it in the header; promote it to the
opening element now that the brand/view are gone.

### `N of M agents` count — **keep, simplify the wording**

The "visible vs total" signal is load-bearing during filter work
(operator filters by harness, the count drops, they can see what they
hid). Keep the data; tighten the format. Current: `122 of 118 agents`
(note: 122 > 118 because expanded mux-candidate child rows count as
visible). Proposed: `122/118 sessions` — saves 6 cells and the word
"sessions" matches the surface name better than "agents" (the rest of
the TUI uses "sessions" for the row kind; "agents" is an Atelier-era
relic). Use just `N sessions` when filtered == total to keep the
common case minimal.

### `M mux` count — **keep**

Useful context the operator can't read off any single row. Six cells
is cheap.

### Per-harness chips (`[claude] 50  [codex] 46  [opencode] 26`) — **drop default; opt-in flag**

The story called this question explicitly. Looking at the evidence:

- **Row badges already carry per-session harness identity.** Every
  agent-session row in the tree shows its harness badge inline; the
  operator reads "claude" off every claude row without looking up.
- **Group summary chips carry per-group totals.** Group rows show
  `(N)` aggregating their descendants; for "how many sessions in
  this project?" the answer is already in the row tree.
- **Aggregate per-harness counts are useful but not load-bearing.**
  "I have 50 claude sessions across everything" is occasionally
  interesting, but the operator who needs that number can filter to
  `harness:claude` and read the visible count.

Drop from default render. The 40+ cells they consume at wide widths
are the primary reason the header overflows; reclaiming them is the
biggest single win. Add an opt-in `[tui] show_harness_chips = true`
config knob (not a CLI flag — operators set their preference once)
for the narrow set of operators who genuinely want the aggregate at a
glance. Default false. ADR 0032's flat-key theme model is the
template for the toggle.

### Mux-state chips (`◉ 10  ◐ 0  ◯ 112`) — **collapse to ambiguity-only**

ADR 0072 made the per-row chip binary (attached vs everything else)
and group rows own the `⚠` ambiguity glyph. The three-bucket
aggregate in the header was sized before that shift. Re-evaluating:

- `◉ 10` (attached) — operator reads `◉` off every attached session
  row; aggregate is not load-bearing.
- `◯ 112` (unmuxed) — derives from `visible_sessions − ◉ − ◐`;
  not new information.
- `◐ N` (ambiguous) — **this is the actionable one.** When N > 0,
  the operator needs to know to triage. When N = 0, the cell is
  paying for nothing.

Collapse the three chips to one. Render `⚠ N` (using ADR 0072's `⚠`
vocabulary, not the `◐` glyph — the row-tree chip vocabulary already
chose `⚠` for ambiguity) **only when N > 0**, after the agent count
section. When N = 0, render nothing — the header gets shorter, which
is the right outcome.

### View tab strip on left-panel title — **keep where it is**

The story asked whether it should move to a dedicated row so the top
header can shrink to one line on narrow terminals. The header is
already one line; the strip is on a border row, not a content row,
so it doesn't compete with body height. Moving it would burn a
content row for no clear win, and the active-view signal would
either duplicate or relocate from a stable position. Keep it.

## Proposed header after the audit

Same 160-col frame, after the audit:

```
updated 0s ago · 122/118 sessions · 11 mux
```

At 80 cols (the H-UI series' target):

```
updated 0s ago · 122/118 sessions · 11 mux
```

At 80 cols with ambiguity present:

```
updated 0s ago · 122/118 sessions · 11 mux · ⚠ 2
```

With operator opt-in `show_harness_chips`:

```
updated 0s ago · 122/118 sessions · 11 mux · [claude] 50 [codex] 46 [opencode] 26 · ⚠ 2
```

Width reclaimed at default: ~70 cells (Conspectus + sessions + harness
chips + two of three mux chips). The header now fits comfortably on
80-col terminals with room to spare, which was the original H-UI
series' narrow-pane target.

## Cross-view question (open in the story)

> Should the redesign also cover the `mux` / `union` / `prs` / `forks`
> views (their headers re-use the same composition) or scope strictly
> to sessions?

Recommendation: **apply the redesign to every view in the same PR.**
The header composition is shared (one `draw_header` function) — the
view-specific data flows in via `view_label`, the count is generalized
to "rows of the view's primary kind," and the mux chips already
collapse cleanly. Per-view divergence would be churn; the audit's
deletions apply equally across all views.

The one wrinkle is per-view count language: `sessions` is the natural
word for the Sessions view, but `pull requests` / `forks` / etc. for
the others. The `N/M sessions` format generalizes to
`N/M <primary-kind>`. Resolver: a small `header_primary_kind_word(view)
-> &'static str` helper.

## Implementation plan

If the operator approves the audit:

1. **Plumbing — config knob** (~30 LOC): add `[tui] show_harness_chips:
   bool` to `RunConfig` (default `false`), wire it through to
   `draw_header` via `app.config()`.
2. **Header rewrite** (~60 LOC net, mostly deletions): rewrite
   `draw_header` to emit the new sequence. Replace the bold prefix
   format with the new wording; replace the three-bucket mux chip
   block with a single `⚠ N` chip rendered only when N > 0; gate the
   harness chips on `show_harness_chips`. Add `header_primary_kind_word`
   for cross-view generalization.
3. **Snapshot regeneration** (~3 snapshot fixtures): regenerate
   `--snapshot --snapshot-pane header` at 160 / 100 / 70 cols, plus
   variants under `show_harness_chips=true` and `⚠ N` present.
4. **Test pinning** (~5 unit tests in `src/tui/ui.rs:tests`): one per
   element kept (freshness present/absent, count filtered/equal,
   ambiguity present/absent, harness chips on/off). Snapshot tests for
   the visual.
5. **ADR**: the audit's deletions don't change architecture, but they
   do establish a "header carries identity + load-bearing counts; row
   tree carries per-row signals; status bar carries contextual hints"
   rule that's worth memorializing. Tentatively ADR 0078 — record
   alongside the implementation PR.

Estimated total: ~120 LOC delta + 3–4 snapshot fixtures + ADR.

## Decision points awaiting approval

Before any code change lands:

1. **Drop the `Conspectus` brand and `sessions` view label from the
   header prefix?** (Recommended: yes — both duplicate stronger signals
   elsewhere.)
2. **`N/M sessions` vs `N of M agents`?** (Recommended:
   `N/M sessions` — matches surface vocabulary, saves 6 cells.)
3. **Per-harness chips: drop default, add `show_harness_chips` opt-in?**
   (Recommended: yes — biggest single width reclamation, opt-in
   preserves the use case for operators who want the aggregate.)
4. **Mux-state chips: collapse to ambiguity-only `⚠ N` when N > 0?**
   (Recommended: yes — `◉` and `◯` are derivable from row tree;
   only `⚠` is actionable.)
5. **Freshness: keep in header or move to status bar?** (Recommended:
   keep — status bar already carries contextual hints that change
   with focus; freshness is a primary content signal.)
6. **Apply to all views or sessions-only?** (Recommended: all views;
   shared composition makes per-view divergence churn.)
7. **ADR 0078 to memorialize the "header / row tree / status bar"
   division of labor?** (Recommended: yes — small ADR alongside the
   PR.)
