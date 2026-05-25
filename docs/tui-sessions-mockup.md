# `conspectus tui` Sessions View — Mockup For Review

This document is a **review-and-refine artifact**, not a contract. It
sketches what the v1 default sessions view looks like for the
Returning Operator journey (Phase 8, P8-001), populated with real
session state from the author's machine.

The plan-of-record stays in
[`docs/implementation/phase-08-interactive-tui.md`](implementation/phase-08-interactive-tui.md);
this mockup feeds into it as the source for the locked layout and
behavior decisions captured at the bottom.

## What's In The Snapshot

Real state captured from this machine on 2026-05-19:

- **24 claude-code sessions** across `~/src/conspectus`,
  `~/src/oss/worktrunk`, `~/src/atelier`, `~/src/sysadmin`, and a few
  agent-deck multi-repo checkouts.
- **~22 codex sessions** mostly under `~/src/atelier` and one fork
  checkout.
- **3 opencode sessions** under `~/src/conspectus`.
- **3 tmux sessions** managed by agent-deck.
- **Preview text** sourced live from each adapter
  (`H-PREVIEW-002..004`).

The mockup below trims that down to a representative slice — the
returning operator doesn't need every row visible, just the most
recent activity organized so they can find the session they want.

All paths render with `~` shortening for `$HOME` regardless of
discovery source, so the operator never sees raw `/home/<user>/…`
strings.

## The Mockup (80×24 default terminal)

```
┌─ Conspectus · sessions ─ updated 12s ago · 24 agents · 3 mux ──────────────┐
│ ▼ ~/src/conspectus                       │ codex:…b4fdee8                  │
│   ▼ ~/src/conspectus                     │ harness    codex                │
│     4e90b4 codex:…b4fdee8     2m   ◐     │ cwd        ~/src/conspectus     │
│       could you give me a bit more co…   │ title      fork lineage q&a     │
│     f9f3cc claude:…6b6346f7   17m  ◉     │ mux        — (2 candidates) ⚠   │
│     297135 claude:…e78de1e6   1h   ◉     │ pr         octo/repo#7 (open) ⟳ │
│   ▶ ~/.agent-deck/multi-repo-…           │ lineage    — (no parent)        │
│ ▼ ~/src/atelier                          │                                 │
│     73566c codex:…b4fdee8     5m   ◯     │ ─ preview ───────────────────── │
│       drafting reroll plan for the…      │ Could you give me a bit more    │
│     d26acd codex:…a26d07cb    2d   ◯     │ context? "The fork didn't seem  │
│     f3fe1f codex:…c2700fe7    3d   ◯     │ to work" could mean a few diff- │
│ ▶ ~/src/oss/worktrunk                    │ erent things given recent work: │
│ ▶ ~/.agent-deck/multi-repo-worktrees     │ 1. The fork lineage extraction  │
│ ▶ ~/src/sysadmin                         │ in the harness (recent commits  │
│                                          │ 65424a8, d8f07d…) ▲ scroll: J/K │
├──────────────────────────────────────────┴─────────────────────────────────┤
│ / search  1-5 view  a attach  i copy id  r refresh  ? help  q quit │ gh ⟳  │
└────────────────────────────────────────────────────────────────────────────┘
```

A few things the mockup is demonstrating beyond the basic layout:

- The conspectus group has two checkouts (main + agent-deck multi-repo),
  so the checkout level is shown.
- The atelier group has one checkout, so the checkout level collapses
  away — sessions hang directly off the project row. This is the
  checkout-depth rule (see "Locked Decisions" below).
- The selected session has `2 candidates` for its mux link. The mux
  row in the header shows the ambiguity inline. Expanding the session
  in the tree (covered below) would surface each candidate as its own
  child row.
- The header carries `updated Ns ago` (recency, not cadence) followed
  by discovery counts.
- The `title` row appears in the right-panel header because the
  selected session has one set. Sessions without titles omit the row
  rather than render `— (no title)`.

### Ambiguous-mux expansion

When a session has ≥ 2 `LinkedToMux` candidates, the row becomes
expandable. The mockup shows it inline; the expanded form looks like:

```
▼ 4e90b4 codex:…b4fdee8     2m   ◐
    ◉ editor          tmux:editor          (preferred)
    ◯ scratch         tmux:scratch
```

Each child is a candidate mux node — same kind the resolver already
emits as `LinkedToMux` evidence. The child's mux glyph reflects the
session's relationship to that specific candidate (`◉` for the
resolver-preferred attach target, `◯` for the alternate). Selecting
a child row navigates the right panel to that mux's `node show`;
`a` attaches to *that* candidate, overriding the resolver's
preferred pick without needing to open the reserved `m` modal.

The same depth rule applies as for checkouts: don't expand a
session that has a single, definitive mux link; only allow
expansion when there are ≥ 2 candidates. Sessions with no mux at
all (`◯`) don't get a disclosure triangle either.

This adds no new graph concepts — every candidate already exists as
a `GraphLink` of kind `LinkedToMux`. The tree just renders the
existing evidence as navigable rows. The `m` modal (`P8-014`) remains
the explicit picker for operators who want a focused dialog; the
tree expansion is the passive equivalent for scanning.

## What Each Element Means

### Header bar

```
┌─ Conspectus · sessions ─ updated 12s ago · 24 agents · 3 mux ──────────────┐
```

- Product / view name.
- Freshness signal: time since the most recent successful refresh
  ("updated 12s ago"). Drifts upward whenever a refresh fails or
  the cadence is long, so the operator can tell at a glance whether
  the counts they're looking at are stale.
- Counts: agent rows + mux rows discovered. Lets the operator
  sanity-check "did discovery find what I expected?".

The exact refresh cadences live in `?` help, not in the header.

### Left panel: the row tree

Hierarchy default per `P8-001a`: workspace (none here) → repo →
checkout → agent session. The checkout level renders only when a
project has >= 2 checkouts; with one checkout, sessions hang directly
off the project row.

#### Group rows

```
▼ ~/src/conspectus                       │
  ▼ ~/src/conspectus                     │
```

- `▼` / `▶` indicates expanded / collapsed.
- Two indent levels here because conspectus has multiple checkouts;
  the atelier section below collapses to a single level for the
  same reason in reverse.
- Path is shown with `~` shortening for `$HOME`, full path
  otherwise.
- Groups without an agent session inside collapse silently; only
  groups with content are shown in v1.

#### Session rows

```
4e90b4 codex:…b4fdee8     2m   ◐
f9f3cc claude:…6b6346f7   17m  ◉
```

Four sub-cells per row, left to right:

1. **Short id** (6 chars from `H-TBL-002`). Same id `node show`
   accepts.
2. **Harness label** with the per-harness color from `H-TBL-014`
   (`claude` shortened from `claude-code` to save width).
3. **Recency** — relative age of `last_message_preview` or session
   write time. `2m`, `17m`, `1h`, `2d`. Right-aligned so the column
   reads vertically.
4. **Mux indicator** — `◉` attached to a tmux session (green),
   `◐` attached with ambiguity (yellow), `◯` un-muxed (dim). Color
   carries the primary signal; the glyph stays small.

After the mux indicator, the row uses any remaining horizontal space
for a **dim inline preview** from `last_message_preview`:

```
4e90b4 codex:…b4fdee8     2m   ◐  could you give me a bit more co…
```

Rows stay one physical line tall. Narrow panes simply omit or crop
the inline preview instead of adding a second preview line under the
session.

### Right panel: header + preview

#### Header

```
codex:…b4fdee8
harness    codex
cwd        ~/src/conspectus
title      fork lineage q&a
mux        — (2 candidates) ⚠
pr         octo/repo#7 (open) ⟳
lineage    — (no parent)
```

This is `conspectus node show` in compact form, with three
adjustments:

- `~` shortening on paths.
- The `pr` row shows the async-enrichment loading state with `⟳`
  while the background `gh pr view` is in flight (`P8-012a`). Once
  it returns, the row gains `· checks 4/4 ✓ · reviews 1`.
- The `title` row appears only when the session has a non-empty
  title (e.g. opencode chat topics). When absent, the row is
  omitted rather than rendered as `—` to keep the header field set
  consistent with what's known.

Future work: when a project group contains multiple sessions of the
same harness and `title` uniquely distinguishes them, the title
should also appear in the *tree* row itself, not just the header.
Tracked as `P8-015` in the backlog, dependent on `H-TBL-015`
finishing the AGENT-cell cleanup and on `P8-004` having a stable
row-tree builder to extend.

#### Preview

```
─ preview ──────────────────────
Could you give me a bit more
context? "The fork didn't seem
to work" could mean a few diff-
erent things given recent work:
1. The fork lineage extraction
in the harness (recent commits
65424a8, d8f07d…)  ▲ scroll: J/K
```

The preview content depends on the selected row kind:

- **Un-muxed agent session**: `last_message_preview` from the graph,
  soft-wrapped to the panel width. When the preview pane has room
  for more than the graph-resident snippet (capped at 200 chars per
  ADR 0023) and `--no-live-preview` is not set, the data adapter
  does a transcript-tail read on selection and renders the fuller
  message. The same-line previews in the tree always use the
  graph-resident snippet — only the right-panel preview expands.
- **Muxed agent session**: a tmux `capture-pane` snapshot of the
  attached mux, polled at the mux-preview cadence. The session's
  `last_message_preview` is *not* shown there by default — the
  pane capture is the "what was it doing" answer for muxed
  sessions. The header's mux row carries the attachment context.
- **Standalone mux row**: pane capture, same as above.

Scroll hint on the bottom-right reminds the operator they can scroll
the preview when focus is on the right panel.

### Status bar

```
/ search  1-5 view  a attach  i copy id  r refresh  ? help  q quit │ gh ⟳
```

- Left side: keybinding hints.
- Right side: provider-status chips (`gh ⟳` = `gh` fetch in
  progress; would go red on error per the empty/error states table).
- The chips also surface mux-ambiguity reasons ("3 mux candidates")
  when relevant to the selected row.

## The Returning-Operator Journey Walking Through The Mockup

Day-after-yesterday scenario, walk-through of the keystrokes:

1. **Launch** — `conspectus tui` opens to this exact frame. The
   selected row is the most-recently-active session
   (`4e90b4 codex:…b4fdee8`) because hierarchy-first sort still
   promotes the most recent session within each group's leading
   slot.
2. **Scan** — operator's eye lands on the inline preview ("Could
   you give me a bit more context?"). They remember: that's the
   conversation about the fork lineage they left mid-question
   yesterday.
3. **Decide** — they want to resume it. But this session has two
   mux candidates (`◐`, `mux — (2 candidates) ⚠`). They press
   `Enter` to expand and see the two candidate muxes; the
   preferred one is marked. They could press `a` here to attach
   to the preferred candidate, or `j` to the alternate and `a`
   to override.
4. **Attach** — `a`. The TUI exits, dropping them into the chosen
   tmux session.

Total keystrokes: 1 to launch, 1 to expand, 0–1 to navigate, 1 to
attach. The "under 5 keystrokes" target from the operator-journey
goal in the phase-08 plan holds.

## Locked Decisions From This Review

The questions originally posed as "Things I'm Guessing At" are
resolved. They fold into the phase-08 plan on the next pass.

1. **Tree depth (checkout level)** — show the checkout level only
   when a project has >= 2 checkouts. With one checkout, sessions
   hang directly off the project row. This is rule **(c)** from the
   open question.
2. **Tree depth (mux candidates)** — apply the same rule to a new
   per-session expansion level. A session with a single, definitive
   mux link (or none) is a leaf. A session with ≥ 2 `LinkedToMux`
   candidates becomes expandable; expanding shows each candidate as
   a child row. The resolver's preferred candidate is marked.
3. **Activity indicator** — dropped for v1. The recency column
   already carries the "which one was I in" signal; the `●`/`○`
   prefix was visual noise.
4. **Inline preview density** — session rows stay one physical line
   tall. When horizontal space remains after the mux indicator, show
   a dim same-line preview from `last_message_preview`; otherwise
   omit or crop it. The TUI should be responsive to terminal width,
   not pinned to the 80-col default.
5. **Mux indicator glyph set** — `◉`/`◐`/`◯` with color carrying
   the primary signal (green/yellow/dim). Option **(b)** from the
   open question.
6. **Header bar content** — drop refresh cadences. Show
   `updated Ns ago` (time since the most recent successful refresh)
   alongside the discovery counts. Cadences live in `?` help only.
7. **Right-panel header field set** — keep the existing five
   (harness, cwd, mux, pr, lineage) and add `title` when the
   session has one set. Sessions without a title omit the row
   rather than render `—`.
8. **`title` in the tree (deferred)** — when title uniquely
   distinguishes sessions within a group, it should appear in the
   tree row itself, not only the header. Tracked as `P8-015`.
9. **Inline preview vs `--no-live-preview`** — `--no-live-preview`
   does **not** suppress same-line row previews or the graph-resident
   right-panel preview. It only suppresses the live extras:
   mux pane capture and the transcript-tail read that fills the
   right-panel preview beyond the graph-resident snippet. Option
   **(a)** from the open question.

## Refined In The Styling Overhaul

The styling overhaul (ADR 0032, ADR 0033) refines the mockup above
while preserving every locked decision. Plan-of-record:
`~/.claude/plans/let-s-brainstorm-improvements-to-cozy-bengio.md`.

What the overhaul changed visually:

- **Header is one dense line**: original `Conspectus · sessions ·
  updated 12s ago · N agents · M mux` prefix stays as the BOLD
  identity, with per-harness badge chips and per-mux-state count
  chips appended when terminal width allows (`[claude] 12  [codex]
  8  …  ◉ 2  ◐ 1  ◯ 0`). Width-aware dropoff drops the harness chip
  section first, then the mux chips, before truncating the prefix.
- **Session recency colors by activity bucket** (Fresh < 5m / Active
  < 1h / Recent < 1d / Cold ≥ 1d). The recency text is unchanged
  per locked decision 4 — only the color carries the new freshness
  cue.
- **Harness label renders as a filled chip** (` codex `, REVERSED +
  BOLD over the harness color). Shorter labels get plain trailing
  padding so the recency column stays aligned.
- **Group rows carry agent + mux summary chips** (`(N)  ◉ a ◐ b ◯
  c`). Workspace-only ancestors with no sessions stay quiet.
- **Right-panel detail splits into labeled sections** (Session, Mux,
  PR, Lineage) with right-anchored dividers `────── Session ──`.
  Sections whose fields are all blank placeholders are suppressed,
  so sparse sessions get a shorter pane instead of rows of dashes.
  Per-section field colorization: `cwd` → cyan, titles → bold, PR
  state colors from the ADR 0022 palette, identifiers blue,
  ambiguous-mux annotation yellow.
- **Every color is overridable** under `[tui.theme]` per ADR 0032.
  Unknown keys and malformed values surface as warnings; the TUI
  never aborts on theme errors.

What the overhaul kept locked:

- Single-line session rows (no card layout).
- No fabricated per-session live status (running/waiting/idle). The
  recency color buckets stand in for liveness; real status detection
  is tracked separately as a follow-on backlog item.
- Mux glyph set (`◉`/`◐`/`◯`) and color semantics.
- `updated Ns ago` freshness wording in the header.
- `~`-shortening upstream in row tree and detail view-models.
- Selection highlight via `REVERSED` so it works on both light and
  dark terminals.

## Things The Mockup Doesn't Show

- **Search overlay** (`/`). Probably modal-bottom, prompt-style:
  `> typed-query                                           ▌`.
- **Help overlay** (`?`). Modal full-screen list of keybindings,
  including the static refresh cadences.
- **Empty / loading / error frames** from the table in the phase-08
  doc.
- **Additional wide-terminal preview behavior** beyond same-line
  cropping. The row stays one physical line tall at every width.
- **Card layout / multi-line previews**. Not in v1.
- **PR / fork / mux / union views**. Out of scope per the user's
  ask.
- **Mouse interactions**. Deferred per phase-08.

## Residual Questions (Lower Priority)

These came up in the original mockup and weren't explicitly
addressed; they are not v1-blocking but should be noted before
`P8-007` snapshot tests freeze the renderer.

- Are the recency suffixes (`2m`, `17m`, `1h`, `2d`) clear enough,
  or should rows older than some threshold switch to absolute
  timestamps?
- Should the leftmost glyph column also encode "you are currently
  attached to this session from your present tmux", i.e. a
  self-attach marker distinct from the generic `◉`?
