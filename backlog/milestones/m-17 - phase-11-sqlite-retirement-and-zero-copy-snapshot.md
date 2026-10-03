---
id: m-17
title: "Phase 11: SQLite Retirement And Zero-Copy Snapshot"
---

## Description

Source plan: ADRs 0082 (retire SQLite) and 0083 (zero-copy
snapshot format). Phase goal: replace the SQLite-backed
persistence + query cluster with a daemon-centric in-memory
`GraphSnapshot` plus a single mmap-able on-disk artifact in a
zero-copy format (rkyv). Delete `src/query/`, the `conspectus
query` subcommand, the `query` Cargo feature, and the bundled
libsqlite3 build. The producer pipeline (discovery → resolve →
`GraphSnapshot`) is unchanged; this phase is entirely about the
consumer-side read surface and persistence story.

Driving observations from Phase 7 + 9 + 10 in production:

- Full cold discovery is single-digit seconds at target scale;
  the warm-start cache is not measurably faster for a TUI
  launch (the original justification for SQLite as a warm-start
  store no longer holds).
- The daemon already holds a fully-hydrated `GraphSnapshot`;
  serializing to SQLite rows and reconstructing typed nodes via
  `query::reader` on every consumer call is a marshalling tax
  that exists only to feed the SQL surface.
- `conspectus query <sql>` is a nice-to-have that accounts for
  negligible operator-visible use vs. the ~6500 lines + bundled
  C dep it costs.

Dependency shape inside the phase:

```
CSP-438 (ADR 0082) ──┐
CSP-439 (ADR 0083) ──┤
                     ├──→ CSP-440 (model derives) ──→ CSP-441 (format module) ──→ CSP-442 (daemon dual-write)
                     │                                                              ├──→ CSP-443 (socket snapshot cmd) ──→ CSP-444 (TUI socket cutover)
                     │                                                              └──→ CSP-445 (CLI mmap-or-rebuild) ──→ CSP-446 (daemon warm-start, optional)
                     │
                     │  (after every reader is off SQLite)
                     │
                     └──→ CSP-447 (drop `conspectus query`) ──→ CSP-448 (delete src/query/, drop rusqlite) ──→ CSP-449 (ADR supersession + design.md) ──→ CSP-450 (operator migration notes)
```

`CSP-440`, `CSP-441`, `CSP-442` form the additive landing
sequence; the daemon writes both formats during the dual-write
window so reader cutovers (`CSP-443` → `CSP-446`) can land
incrementally without breaking either side. Only after every
reader is off SQLite does the deletion sequence (`CSP-447` →
`CSP-450`) start; that ordering keeps `main` releasable at every
intermediate commit.

- **CSP-438** ADR: retire SQLite persistence and query surface

- **CSP-439** ADR: zero-copy snapshot format selection

- **CSP-440** Add rkyv archive derives to the graph model

- **CSP-441** Implement the snapshot format module

- **CSP-442** Daemon dual-writes SQLite and the new artifact

- **CSP-443** Add the `snapshot` socket command

- **CSP-444** Cut the TUI refresh over to socket-served snapshots

- **CSP-445** One-shot CLI daemon-or-rebuild path (mmap-fresh deferred)

- **CSP-446** Daemon warm-start from the on-disk artifact

- **CSP-447** Remove the `conspectus query` subcommand

- **CSP-448** Delete `src/query/`, drop the `query` Cargo feature, retire the SQLite read surface

- **CSP-448.01** Retire SQLite persistence layer; daemon + CLI use in-memory snapshot caches and `graph.bin` only

- **CSP-448.02** Restore in-memory rendering for the output crate

- **CSP-448.03** Rewrite TUI row builders for forks / prs / union in-memory

- **CSP-448.04** Delete `src/query/`, drop the `query` Cargo feature, finish in-memory TUI rendering

- **CSP-448.05** Revisit hook sidecar SQLite storage

- **CSP-449** Supersede the SQLite ADR cluster and rewrite design.md

- **CSP-450** Operator migration notes

### Command Search And Minibuffer

The TUI surface (`conspectus tui`) is growing in keybindings, sub-views,
and modal surfaces (viewer, rename, graph export). Users need a
discoverable, keyboard-driven way to find and invoke commands — without
leaving the keyboard or memorizing dozens of arcane chords.

Two complementary approaches, drawn from mature terminal-first ecosystems:

1. **Emacs-like minibuffer**: a single-line prompt at the bottom of the
   screen that accepts text commands with tab completion, history,
   and inline validation. Every TUI action eventually routes through
   it. Inspired by Emacs `M-x` and the classic readline pattern.
2. **OpenCode-style command search modal**: a fuzzy-filtered overlay
   that indexes every available command, action, and sub-view. The
   operator types a few characters, arrows through results, and
   presses Enter. Inspired by VS Code's Command Palette and opencode's
   `/` command surface.

The H-TBL workstream already established a column registry and
`conspectus columns <ROWS>` discovery surface; the TUI command palette
can reuse the same registry-plus-description pattern at the action
level. The text-input primitive (`src/tui/widgets/input.rs`) from
ADR 0030 (CSP-240) provides the base widget. The fuzzy filter can
reuse the structural matching approach from the existing
`node_short_id` prefix resolver or adopt a lightweight substring scorer.

- **CSP-390** ADR: command palette surface and minibuffer scope
- **CSP-391** Action registry: enumerate the command surface
- **CSP-392** Command palette overlay (first deliverable)
- **CSP-393** Minibuffer prompt (second deliverable, deferred)
- **CSP-394** Migrate rename, search, and view-switch prompts into the minibuffer

### Per-Node-Type Visual Identity

The TUI row tree renders nine `GraphNode` / `NodeId` variants, but only
`AgentSession` and `MuxSession` carry a strong visual identity (colored
harness pill badge, mux-state circle glyphs). Nodes that render as group
rows — `Workspace`, `Repo`, `Checkout` — and types that appear only in
detail panels — `Branch`, `RuntimeProcess` — are visually
indistinguishable: every group row is a bold path with a disclosure
glyph, every detail-panel node-kinds chip is a dim `[kind]` label.

Operators scanning a dense sessions tree or a right-panel explorer do
not have a consistently fast way to tell *what kind of thing* a row
represents before reading its label. OpenCode's TUI assigns a per-entity
glyph and color; Emacs `dired` and `ibuffer` use per-type faces; Tmux
uses status-line symbols.

This workstream assigns a stable, terminal-safe visual identity — a
**glyph** (geometric shape / Nerd Font symbol / limited emoji), a
**color** (foreground or badge-fill), and a consistent **placement rule**
— to every `GraphNode` variant, so the operator recognizes node kinds at
a glance regardless of view, grouping, or filter state.

The `Theme` struct (`src/tui/theme.rs`) centralizes all TUI colors
including per-harness and mux-state; node-type colors follow the same
pattern and live under `[tui.theme]` config overrides. Node-type glyphs
live in `src/tui/icons.rs` (or a similar single-source-of-truth module)
so the row renderer, detail panel, graph explorer, and any future
surfaces all read from one definition.

- **CSP-408** ADR: node-type visual identity system
- **CSP-409** Define the per-node-type glyph and color assignments
- **CSP-410** Apply node-kind glyphs and colors to the TUI row tree
- **CSP-411** Apply node-kind glyphs and colors to the detail panel and graph explorer
- **CSP-412** Surface node-kind identity in non-TUI outputs (cross-surface consistency)
- **CSP-413** Docs and snapshot coverage
