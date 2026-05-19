# `conspectus tui` Sessions View — Mockup For Review

This document is a **review-and-refine artifact**, not a contract. It
sketches what the v1 default sessions view looks like for the
Returning Operator journey (Phase 8, P8-001), populated with real
session state from the author's machine. The goal is to give richer
direction before more code lands.

The plan-of-record stays in
[`docs/implementation/phase-08-interactive-tui.md`](implementation/phase-08-interactive-tui.md);
this mockup folds in once the user has annotated it.

## What's In The Snapshot

Real state captured from this machine on 2026-05-19:

- **24 claude-code sessions** across `~/src/conspectus`,
  `~/src/oss/worktrunk`, `~/src/atelier`, `~/src/sysadmin`, and a few
  agent-deck multi-repo worktrees.
- **~22 codex sessions** mostly under `~/src/atelier` and one fork
  worktree.
- **3 opencode sessions** under `~/src/conspectus`.
- **3 tmux sessions** managed by agent-deck.
- **Preview text** sourced live from each adapter
  (`H-PREVIEW-002..004`).

The mockup below trims that down to a representative slice — the
"returning operator" doesn't need every row visible, just the most
recent activity organized so they can find the session they want.

## The Mockup (80×24 default terminal)

```
┌─ Conspectus · sessions ─ graph 30s · mux 2s · 24 agents · 3 mux ────────────┐
│ ▼ ~/src/conspectus                       │ codex:…b4fdee8                   │
│   ▼ /home/malloc47/src/conspectus        │ harness     codex                │
│     ● 4e90b4 codex:…b4fdee8     2m   ◐  │ cwd         ~/src/conspectus     │
│       could you give me a bit more co…  │ mux         — (no attach)        │
│     ○ f9f3cc claude:…6b6346f7   17m  ◉  │ pr          octo/repo#7 (open) ⟳│
│     ○ 297135 claude:…e78de1e6   1h   ◉  │ lineage     — (no parent)        │
│   ▶ /home/malloc47/.agent-deck/multi-…   │                                  │
│ ▶ ~/src/atelier                          │ ─ preview ──────────────────────│
│   ▼ /home/malloc47/src/atelier           │ Could you give me a bit more     │
│     ● 73566c codex:…b4fdee8     5m   ◯  │ context? "The fork didn't seem   │
│     ○ d26acd codex:…a26d07cb    2d   ◯  │ to work" could mean a few diff-  │
│     ○ f3fe1f codex:…c2700fe7    3d   ◯  │ erent things given recent work:  │
│ ▶ ~/src/oss/worktrunk                    │ 1. The fork lineage extraction  │
│ ▶ ~/.agent-deck/multi-repo-worktrees     │ in the harness (recent commits  │
│ ▶ ~/src/sysadmin                         │ 65424a8, d8f07d…)  ▲ scroll: J/K│
├──────────────────────────────────────────┴──────────────────────────────────┤
│ / search  1-5 view  a attach  i copy id  r refresh  ? help  q quit │ gh ⟳  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## What Each Element Means

### Header bar

```
┌─ Conspectus · sessions ─ graph 30s · mux 2s · 24 agents · 3 mux ────────────┐
```

- Product / view name.
- Refresh cadence as currently configured (helps the operator know
  when data is stale).
- Counts: agent rows + mux rows discovered. Lets the operator
  sanity-check "did discovery find what I expected?".

### Left panel: the row tree

Hierarchy default per `P8-001a`: workspace (none here) → repo →
worktree → agent session.

#### Group rows

```
▼ ~/src/conspectus                       │
  ▼ /home/malloc47/src/conspectus        │
```

- `▼` / `▶` indicates expanded / collapsed.
- Two indent levels here: the repo common-dir, then the worktree
  root. With only one worktree per repo (the common case) the doubled
  indent looks redundant — see "Open questions" below.
- Path is shown with `~` shortening for `$HOME`, full path
  otherwise.
- Groups without an agent session inside collapse silently; only
  groups with content are shown in v1.

#### Session rows

```
● 4e90b4 codex:…b4fdee8     2m   ◐
○ f9f3cc claude:…6b6346f7   17m  ◉
```

Five sub-cells per row, left to right:

1. **Activity indicator** — `●` for the most-recently-touched
   session per group, `○` for the rest. Filled glyph helps the eye
   land on the operator's likely next-attach target.
2. **Short id** (6 chars from `H-TBL-002`). Same id `node show`
   accepts.
3. **Harness label** with the per-harness color from `H-TBL-014`
   (`claude` shortened from `claude-code` to save width).
4. **Recency** — relative age of `last_message_preview` or session
   write time. `2m`, `17m`, `1h`, `2d`. Right-aligned so the column
   reads vertically.
5. **Mux indicator** — `◉` attached to a tmux session, `◐` attached
   with ambiguity (the `*` marker today), `◯` un-muxed.

Below the session row, when space allows, a **dim inline preview**
shows the first ~60 chars of `last_message_preview`. This is the
"what was it doing?" answer that the operator wants without having
to move selection.

```
● 4e90b4 codex:…b4fdee8     2m   ◐
  could you give me a bit more co…
```

Only the selected row, and the most-recent row in each group, get
the inline preview by default. Otherwise the rows pack tighter.

### Right panel: header + preview

#### Header

```
codex:…b4fdee8
harness     codex
cwd         ~/src/conspectus
mux         — (no attach)
pr          octo/repo#7 (open) ⟳
lineage     — (no parent)
```

This is `conspectus node show` in compact form, with two adjustments:
- `~` shortening on paths.
- The `pr` row shows the async-enrichment loading state with `⟳`
  while the background `gh pr view` is in flight (`P8-012a`). Once
  it returns, the row gains `· checks 4/4 ✓ · reviews 1`.

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

- Full `last_message_preview` (capped at 200 chars per ADR 0023),
  soft-wrapped to the panel width.
- For attached sessions, this is also where the mux pane capture
  would land (`P8-009`), polled at 2s.
- Scroll hint on the bottom-right reminds the operator they can
  scroll the preview when focus is on the right panel.

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
2. **Scan** — operator's eye lands on the `●` and the inline
   preview ("Could you give me a bit more context?"). They
   remember: that's the conversation about the fork lineage they
   left mid-question yesterday.
3. **Decide** — they want to resume it. But this session is
   un-muxed (`◯` indicator, `mux — (no attach)` in the header). So
   `a` is disabled. The status bar would surface:
   `disabled: no mux attached. R reserved for resume-into-mux.`
4. **Alternative** — they spot `f9f3cc claude:…6b6346f7` two rows
   below, which is muxed (`◉`). They `j` down to it. The header
   re-renders against the new selection; the preview pane shows
   the claude-code session's last message
   (`[Request interrupted by user]`).
5. **Attach** — `a`. The TUI exits, dropping them into the tmux
   session.

Total keystrokes: 1 to launch, 2 to navigate, 1 to attach. The
"under 5 keystrokes" target from the operator-journey goal in the
phase-08 plan holds.

## Things I'm Guessing At (Direction Welcome)

These are concrete details the locked decisions didn't yet pin, that
I'd want you to weigh in on before they get hardcoded.

### 1. Tree depth: collapse single-worktree repos?

The mockup shows two indent levels for repos with one worktree
(repo → its only worktree → sessions). When the worktree root equals
the repo common-dir's parent (the common case), the second level is
visually redundant.

Options:

- **(a) Always show worktree level** — consistent depth, predictable.
- **(b) Collapse the worktree level when there's only one** —
  flatter for the common case, but the level reappears as soon as a
  fork or linked worktree exists. Operators may find the inconsistent
  depth jarring.
- **(c) Show worktree level only when ≥ 2 worktrees** — same as (b)
  but framed as a rule, not a coincidence.

My current lean: **(c)**, with a config knob if it turns out
inconsistency annoys people.

### 2. Activity indicator: per-group vs per-session

The `●` / `○` distinction in the mockup is per-group ("this is the
most-recent in this repo"). Alternatives:

- **(a) Per-group most-recent** — what the mockup shows. Helps the
  eye locate "the one I was in" within a group at a glance.
- **(b) Global most-recent** — only one `●` in the whole tree,
  everything else `○`. Less noise, but the operator has to scroll
  to find it.
- **(c) Drop the indicator entirely** — the recency column already
  carries the info.

My current lean: **(a)**.

### 3. Inline preview density

The mockup shows the inline preview only for the selected row and
the most-recent row per group. Alternatives:

- **(a) Selected + most-recent-per-group** (mockup default).
- **(b) Selected only** — densest tree, but the most-recent-row
  preview is useful for scanning without moving selection.
- **(c) All rows** — most info, but the tree becomes hard to scan
  at 24 rows of terminal height.
- **(d) None in the tree; only in the right panel** — keeps the
  left panel tight; matches the `conspectus table sessions`
  layout. Costs the operator a `j` keystroke per row to see what
  each was doing.

My current lean: **(a)**.

### 4. Mux indicator glyph set

The mockup uses `◉` / `◐` / `◯` for attached / ambiguous /
un-muxed. Wide-enough across fonts but no terminal-color signal.
Alternatives:

- **(a) Glyph trio** (mockup).
- **(b) Color + glyph** — `◉` in green / yellow / dim per state.
- **(c) Text shorthand** — `[mux]` / `[mux*]` / `[—]`. Wider but
  unambiguous in mono fonts.

My current lean: **(b)** — color carries the signal, glyph stays
small.

### 5. Header bar content

The mockup shows refresh cadences and counts inline. Alternatives:

- **(a) Cadences + counts** (mockup).
- **(b) Counts only; cadences in `?` help only**.
- **(c) Cadences only; counts redundant with the tree itself**.

My current lean: **(b)** — cadences are static info, counts are
the operator-relevant signal.

### 6. Right-panel header field set

The mockup shows 5 fields (harness, cwd, mux, pr, lineage). Some
ideas:

- Drop fields that are `—`? — quieter for un-muxed sessions but the
  field set varies per row, which is harder to scan.
- Show declared-link state when present? — would surface a row's
  declared-link override here rather than only via `node show`.
- Show `title` when set (opencode chat topics)? — would be the
  natural home for it after `H-TBL-015` moved it out of the AGENT
  cell.

My current lean: keep the 5 fields shown, **add `title` when set**.

### 7. Inline preview when `--no-live-preview` is set

The flag suppresses mux pane capture and transcript reading. But
`last_message_preview` is graph-resident and was populated at
discovery time. Two reasonable behaviors:

- **(a)** `--no-live-preview` doesn't affect the inline previews —
  they keep rendering. Only the pane-capture and transcript-tail
  preview in the right panel goes blank.
- **(b)** `--no-live-preview` also suppresses the inline previews —
  the whole "what was it last doing?" surface goes dark for
  privacy.

This matches the v1-deferrable question already in the plan but
shows up in the mockup. My current lean: **(a)** because the field
is already on the JSON graph; if the operator wants total
suppression they can disable the harness adapter via env var.

## Things The Mockup Doesn't Show

- **Search overlay** (`/`). Probably modal-bottom, prompt-style:
  `> typed-query                                           ▌`.
- **Help overlay** (`?`). Modal full-screen list of keybindings.
- **Empty / loading / error frames** from the table in the phase-08
  doc.
- **Card layout / multi-line previews**. Not in v1.
- **PR / fork / mux / union views**. Out of scope per the user's
  ask.
- **Mouse interactions**. Deferred per phase-08.

## Open Questions Left For You

Concrete things to comment on, in priority order:

1. Tree depth — collapse single-worktree repos? (Question 1 above.)
2. Activity-indicator scope — per-group, global, or drop. (Q2.)
3. Inline-preview density — when do rows get the dim preview line.
   (Q3.)
4. Anything else in the right-panel header you'd want by default
   that isn't there? (Q6.)
5. Are the recency suffixes (`2m`, `17m`, `1h`, `2d`) clear, or
   should they be absolute times for older entries?
6. Should the leftmost glyph column also encode "session is the
   one you ran most recently inside this tmux session" — i.e.
   "you're attached to this one right now"?

Once you annotate, I'll fold the locked answers into the phase-08
plan and the backlog stories (especially `P8-004` for the tree
builder and `P8-007` for the render contract).
