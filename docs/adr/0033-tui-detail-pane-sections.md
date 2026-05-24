# ADR 0033: TUI Detail-Pane Section Model

## Status

Accepted

## Context

The Phase 8 right-panel detail (`src/tui/detail.rs`'s `NodeDetail`)
ships a flat `Vec<HeaderField>` followed by an undifferentiated
preview stream. Each `HeaderField` is `{ label, value, placeholder,
annotation }`; the renderer draws them as `label    value annotation`
rows under a single bold identity line, then a `─ preview ─` rule, then
the ANSI / transcript content.

The flat schema served the v1 mockup (`docs/tui-sessions-mockup.md`)
well enough, but the [styling overhaul plan](../../plans/let-s-brainstorm-improvements-to-cozy-bengio.md)
calls for an agent-deck-style detail pane with multiple labeled,
visually-separated sections (`────── Session ──`,
`────── Mux ──`, `────── PR ──`, `────── Preview ──`,
`────── Output ──`). The TUI review (`docs/tui-review.md`) already
flagged the underlying recognition problem:

> Render detail fields in priority order: 1. action state /
> 2. project/cwd and harness / 3. title or last meaningful session
> label / 4. PR/branch/fork context when present / 5. raw ids last,
> dimmed or hidden behind a copy action.

A flat `HeaderField` list cannot carry the structural metadata a
sectioned renderer needs: which field belongs to which section, when a
section is suppressed entirely vs rendered with a placeholder, and how
field-level colorization (PR state green/red/magenta, mux ambiguity
yellow, cwd cyan, id blue) maps onto each section's contents.

Three questions are large enough to ADR rather than embed in the
overhaul plan's Phase 6:

1. **Section model shape** — fixed enum, label-keyed map, or
   per-node-kind bespoke layout.
2. **Suppression rule** — when does a section omit vs render an
   empty placeholder.
3. **Colorization mapping** — how `Theme` values (ADR 0032) attach to
   field values per section, given that the same field label (e.g.
   `cwd`) can appear in multiple sections.

The decisions cascade into the `NodeDetail` shape, the snapshot test
matrix (full vs sparse), and the renderer's divider logic.

## Decision

### 1. Section as a fixed enum, not a label-keyed map

Replace `NodeDetail::header_fields: Vec<HeaderField>` with a
`Vec<DetailSection>`:

```rust
pub struct DetailSection {
    pub kind: SectionKind,
    pub fields: Vec<HeaderField>,
}

pub enum SectionKind {
    Session,    // harness, cwd, title, model
    Mux,        // attachment, candidate count, backend native id
    Pr,         // state, draft, checks, reviews, branch
    Lineage,    // parent session, fork relation, branch
    Preview,    // ANSI pane capture or transcript tail
    Output,     // post-preview content (commit summaries, tool output)
}
```

The enum is closed in v1. Adding a new section kind is an explicit
code change with a snapshot update, not a config-time decision.

`HeaderField` keeps its existing shape — only its container changes.
The renderer walks sections in enum-declaration order; each section
emits an optional divider header followed by its field rows.

Reasons for a closed enum over a label-keyed map:

- The set of detail facets is small (~6) and project-rooted; new ones
  arrive with new graph node kinds, not with new config.
- A closed enum gives the renderer compile-time exhaustiveness on
  divider-label lookup, ordering, and per-section colorization
  switches.
- The CLI `node show` command can reuse the same enum to print the
  same section structure as plain text in a follow-on story without
  re-deriving the layout from string labels.

The `LinkSummary` / `ResolvedSummary` / `DiagnosticSummary` fields on
`NodeDetail` are unchanged. They render below the sectioned header
when the operator scrolls past it (existing behavior). A future ADR
may fold them into a `SectionKind::Links` if operators ask.

### 2. Suppression rule — omit when the section has no signal

A section is **omitted entirely** (no divider, no rows) when:

- it would contain zero fields, **or**
- every field in it would be a placeholder (`HeaderField.placeholder
  == true`) with no annotation.

A section with at least one non-placeholder field renders fully,
including any placeholder rows it contains (dimmed per the existing
rule). The intent: an empty Mux section disappears for un-muxed
sessions, but a Mux section with one resolved attachment + one
placeholder candidate still shows both rows so the operator sees the
full picture.

This generalizes the existing "omit `title` row when absent" rule
from `docs/tui-sessions-mockup.md` (locked decision 7) to the section
level. Sessions without a PR, without lineage, or without a mux link
get a shorter detail pane rather than rows of `—` placeholders.

The `Preview` and `Output` sections follow the same rule with one
addition: `Preview` is suppressed when the preview store yields no
content for the selected row (e.g. mux capture failed). The renderer
surfaces the failure on the status bar (existing behavior), not as a
visible empty divider.

### 3. Colorization mapping — per-section + per-field-label

Field-value colorization is the section's responsibility, applied at
render time. The mapping is:

| Section | Field label    | Color source (from `Theme`, ADR 0032) |
| ------- | -------------- | -------------------------------------- |
| Session | `harness`      | `theme.harness_*` (badge rendering)   |
| Session | `cwd`          | `theme.cwd_mark` (cyan path)          |
| Session | `title`        | bold (no fg color)                    |
| Session | `model`        | default fg                            |
| Mux     | `attachment`   | `theme.mux_attached` / `mux_ambiguous`|
| Mux     | `candidates`   | `theme.warning` when count ≥ 2        |
| Mux     | `backend`      | `theme.link_id` (blue native id)      |
| Pr      | `state`        | `pr_open`/`pr_closed`/`pr_merged`/`pr_draft` |
| Pr      | `checks`       | `theme.success` / `theme.error`       |
| Pr      | `reviews`      | default fg                            |
| Pr      | `branch`       | default fg                            |
| Lineage | `parent`       | `theme.link_id` (id), default text    |
| Lineage | `branch`       | default fg                            |
| any     | placeholder=true | `theme.placeholder` (dim)           |
| any     | annotation     | `theme.warning` for ⚠, dim for `(preferred)` |

The same label may colorize differently in different sections (e.g.
`branch` in PR vs `branch` in Lineage). This is why the mapping is
per-section, not per-label. The renderer dispatches on
`(SectionKind, field.label)` using a `match` in `src/tui/ui.rs`'s
detail rendering function.

The identity line at the top of the detail pane (currently bold
title, e.g. `codex:…b4fdee8`) becomes a chip row reusing the harness
badge widget (overhaul plan Phase 4) — the line above all sections,
not inside `Session`.

### 4. Divider rendering

Section dividers use a right-anchored label:

```
──────────────────────────── Session ──
```

- The rule character is `─` (`U+2500 BOX DRAWINGS LIGHT HORIZONTAL`),
  matching the existing single-pane `─ preview ─` divider.
- The rule is rendered with `theme.divider` (default `DIM`).
- The label is rendered with default fg + bold modifier.
- A trailing `──` (two cells) anchors the label visually.
- The label sits ~6 cells from the right edge of the panel; the
  leading rule fills the remaining width.
- When the panel is narrower than `~label_len + 10`, the label
  drops to a centered short divider (`─── Session ───`) to preserve
  scannability. Sections never wrap their dividers.

The first section in the pane omits the leading divider; the chip
identity row + a single blank line serves as the visual top edge.
Sections after the first always emit their divider.

### 5. Plumbing

`build_node_detail` in `src/tui/detail.rs` returns a `NodeDetail` with
the new `sections: Vec<DetailSection>` field replacing
`header_fields`. The per-node-kind builders (`agent_section_fields`,
`mux_section_fields`, etc.) compose into one or more sections
according to the field labels they emit. The function remains pure
and snapshot-testable.

The renderer in `src/tui/ui.rs` walks sections, applies the
suppression rule, emits the divider, and renders each field through
the colorization mapping above. The `Preview` and `Output` sections
wrap the existing `PreviewContent` and ANSI-rendered text — no
behavior change inside those sections, only the labeled divider above
them.

No new dependencies. The change is internal to the TUI module; no
public API surface is affected.

## Consequences

- The detail pane gains the agent-deck-style visual structure called
  for in the overhaul plan and in the TUI review's recommendations.
- Sessions with sparse metadata (no PR, no mux, no lineage) get a
  shorter detail pane instead of three rows of placeholders. The
  pane "shrinks to fit" the available signal.
- Snapshot test surface widens: each `SectionKind` permutation
  becomes a snapshot fixture (full, sparse, single-section,
  preview-missing). The matrix grows from ~3 snapshots to ~8.
- Adding a new section kind is a focused code change: add the enum
  variant, add the divider label, add the per-section colorization
  rows, update snapshots. No config schema migration.
- The `node show` CLI command can adopt the same section structure
  later as plain-text output, giving operators a consistent mental
  model across CLI and TUI surfaces (filed as a follow-on backlog
  item under T8-024's umbrella, not in scope here).
- Where this ADR overlaps with ADR 0032: section dividers and field
  colorization all read from `Theme`. The two ADRs ship together as
  part of the styling overhaul; either can be reverted independently
  but neither delivers full value alone.

## Alternatives Considered

- **Label-keyed map (`HashMap<&'static str, Vec<HeaderField>>`).**
  Rejected. Loses compile-time exhaustiveness, makes ordering
  ambiguous, and would require duplicating divider labels in code +
  data.
- **Per-node-kind bespoke layouts** (e.g., `AgentDetail`,
  `MuxDetail`, `PrDetail` structs). Rejected. The graph already has
  many node kinds (workspace, repo, checkout, branch, fork, agent
  session, mux session, PR, …); per-kind layouts would explode the
  surface and lose the cross-kind regularities the section model
  captures.
- **Keep flat `Vec<HeaderField>`, add a `section: &'static str`
  field.** Rejected. The string field re-introduces label-keyed
  fragility; the closed enum is the same information with type
  safety.
- **Render placeholders for every section even when absent.**
  Rejected. The mockup's locked decision 7 already established the
  "omit when absent" rule for individual fields; extending it to
  sections is the consistent generalization.
- **Tabs for the detail pane** (Session | Mux | PR tabs the operator
  switches between). Rejected. The TUI review explicitly called out
  tabs as slowing the primary attach path; sections are the
  always-visible alternative.
- **Section ordering driven by node kind** (e.g., put Mux first for
  ambiguous sessions). Rejected for v1. Stable ordering is a
  scannability win — the operator's eye learns the layout. A future
  ADR may add an "emphasize" rule that promotes a section without
  reordering.

## Open Questions Answered

- The detail pane has a closed `SectionKind` enum, not a label-keyed
  map or per-node-kind structs.
- Sections omit entirely when they would contain only placeholders;
  otherwise they render fully (placeholders included, dimmed).
- Field colorization is dispatched on `(SectionKind, label)` from a
  `Theme` value (ADR 0032), not from per-field hardcoded literals.
- The first section omits its leading divider; subsequent sections
  always render theirs. Dividers degrade to centered short rules in
  narrow panels.
- `LinkSummary` / `ResolvedSummary` / `DiagnosticSummary` are
  unchanged in v1; a future ADR may fold them into a section.
- The change is internal to the TUI module; no public API or
  dependency surface is affected.
