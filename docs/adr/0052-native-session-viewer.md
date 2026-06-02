# ADR 0052: Native In-Tree Multi-Harness Session Transcript Viewer

## Status

Accepted. Supersedes the *default* viewer path established by ADR
0019 (external launch) for Claude Code, Codex, and OpenCode
sessions. External launches remain available as an escape hatch.

## Context

ADR 0019 (amended May 2026) and `H-TRANSCRIPT-012` shipped an
external-viewer launch path: press `T` on an `AgentSession` row,
suspend the TUI, hand control to `claude-history` (Claude Code) or
patched `recall` (multi-harness). Operating that path against real
operator data exposed three real gaps:

1. **OpenCode coverage doesn't exist on the operator's machine.**
   Recall's `OpenCodeParser` is structurally file-walk based and
   expects `~/.local/share/opencode/storage/session/<proj>/ses_*.json`.
   On at least one observed machine, that path is empty: OpenCode
   lives in `opencode.db` (SQLite) with `session_diff/` as the only
   on-disk artifact. Recall returns "Session not found" regardless
   of what flags we pass. Restoring coverage requires teaching
   recall a SQLite reader path (~250-400 LOC fork patch, uncertain
   upstream-PR acceptance).
2. **Claude-history is Claude-only.** Mature, but covers exactly
   one harness.
3. **Recall debt is compounding.** The conspectus Nix overlay
   already carries: a custom from-source build, a
   `[patch.crates-io] crossterm` redirect to deal with a
   same-version git/registry collision, a vendored `Cargo.lock`,
   the `--session <ID>` patch, a `BinaryProbe::supports_flag`
   capability gate, and a `recall 0.5.0-conspectus-session`
   version bump. Each upstream release adds re-vendor + re-hash
   + re-patch work. Open recall-side stories (`H-TRANSCRIPT-014`
   opencode coverage, `H-TRANSCRIPT-015` j/k focus,
   "initial-scroll-to-bottom") would each add another patch hunk
   on top, with no guarantee any land upstream.

Conspectus already owns much of what an in-tree viewer would need:
the per-harness parsers (Claude Code JSONL, Codex JSONL, OpenCode
SQLite via ADR 0013), the H-PREVIEW extractors that already grammar-
correctly skip tool-use / tool-result / thinking / channel-marker
records, the Ratatui runtime and theme (ADRs 0024, 0032), the
`tui-markdown` rendering path (ADR 0051), and the `ansi-to-tui`
fallback (ADR 0025) for tool-output styling.

`H-TRANSCRIPT-003 .. H-TRANSCRIPT-010` were already going to land
a recent-history adapter + an inline preview widget for the right
panel. Growing the same widget into a full-screen scrollable
modal — with the same data layer underneath — is roughly 50% more
code than just the inline preview. In exchange the project drops
all recall debt, OpenCode works because we read SQLite directly,
no upstream coordination is required, no patches need re-applying
on every release, and no `supports_flag` gymnastics are needed on
the conspectus side.

## Decision

Build a native multi-harness session transcript viewer in-tree as
the **default** target of the `T` keybind on `AgentSession` rows.
Render a full-screen scrollable Ratatui modal over the existing
two-panel layout; data comes from per-harness parsers that read
each harness's state-of-record directly (JSONL for Claude Code and
Codex, SQLite for OpenCode per ADR 0013).

The external viewer launch (`H-TRANSCRIPT-012`) is retained as an
**opt-in escape hatch** for operators who prefer `claude-history`'s
ledger formatting, search affordances, or future external viewers.
ADR 0019 is amended accordingly. The `H-TRANSCRIPT-013` config-
override story (deferred) becomes the seam users would use to
swap the default if they want.

### Extraction-ready boundary

The viewer is designed so it can be lifted out of conspectus as a
standalone CLI tool later (`conspectus-transcript-viewer` or
similar). That intent shapes three rules:

1. **One module, no upward imports.** All viewer code lives under
   `src/viewer/`. The module imports *zero* symbols from
   `crate::model`, `crate::resolve`, `crate::query`,
   `crate::discovery`, or `crate::tui::*` *outside* its own
   subtree. Allowed crate-internal exception: `crate::tui::theme`
   when it's clearly factored as a sub-crate-ready palette type
   (revisit if it grows project-specific assumptions).
2. **Own its data model.** The viewer defines `TranscriptTurn`,
   `TranscriptDocument`, and per-harness `SessionLocator` types
   inside `src/viewer/model.rs`. The viewer does *not* take
   `AgentSessionNode`, `NodeId`, or any conspectus-graph type as
   input. The conspectus TUI's adapter (`src/tui/viewer_bridge.rs`,
   say) translates from conspectus's graph types into the viewer's
   locator types at the call boundary.
3. **Own its parsers.** Per-harness transcript readers live under
   `src/viewer/parser/{claude_code, codex, opencode}.rs`. The
   conspectus discovery layer (`src/discovery/harness/*`) is
   refactored — over time, not in one go — to consume the viewer's
   parser primitives where they overlap with discovery's needs.
   Until that refactor completes the parsers are duplicated; the
   viewer's copies are the source of truth for transcript
   semantics.

The module's `mod.rs` carries a docstring that names every direct
external crate it imports. Adding a new dep to the viewer module
without updating that docstring (and `docs/transcript-viewer-deps.md`)
is a review-blocker.

### Module layout

```
src/viewer/
  mod.rs              — public entry point + dep-surface manifest docstring
  model.rs            — TranscriptTurn, TranscriptDocument, SessionLocator
  parser/
    mod.rs            — HarnessParser trait + dispatch
    claude_code.rs    — Claude Code JSONL → TranscriptDocument
    codex.rs          — Codex JSONL → TranscriptDocument
    opencode.rs       — opencode.db SQLite → TranscriptDocument
  widget.rs           — Ratatui full-screen modal: layout, scroll, search
  state.rs            — ViewerState struct (pure, snapshot-testable)
  input.rs            — Key → message reducer (Elm-style per ADR 0024)
  render.rs           — TranscriptTurn → ratatui::text::Text using tui-markdown
  theme.rs            — palette wrapper (re-exports / wraps crate::tui::theme until extraction)

src/tui/viewer_bridge.rs (lives outside the viewer module on purpose)
  — translates AgentSessionId → SessionLocator
  — translates the T keybind into the viewer's open call
  — holds the modal in the runtime's event loop
```

### Dependency surface

The viewer module is permitted these direct crate dependencies
(versions tracked at adoption time; bumps flow through normal
review):

| Crate | Why | Notes |
|---|---|---|
| `ratatui` | Buffer + widget primitives, immediate-mode render | ADR 0024; `default-features = false`, `crossterm` feature |
| `crossterm` | Terminal events (re-exported by ratatui's feature) | Implicit via ratatui; explicit when extracted |
| `tui-markdown` | Per-turn body Markdown → styled `Text` | ADR 0051; `default-features = false` (no syntect / second ansi-to-tui path) |
| `ansi-to-tui` | Tool-output ANSI rendering when shown | ADR 0025; `default-features = false` |
| `rusqlite` | OpenCode session reader | ADR 0013; `bundled` feature so the extracted crate stays single-binary |
| `serde` / `serde_json` | JSONL parsing for Claude Code & Codex | Both `derive` |
| `anyhow` / `thiserror` | Errors at the seam vs. internal | Allow both per crate convention |
| `chrono` *(new)* | Turn timestamps (when shown). Already an indirect dep via rusqlite features; lift to a direct import. | `default-features = false`, `serde` if timestamps are serialized |
| `clap` | Standalone-CLI binary wrapper only (`#[cfg(feature = "bin")]`) | Not imported by the library surface |
| `unicode-width` | Wrap / truncate logic for variable-width content | Already a project dep |

Explicitly **not** in the viewer dep surface:

- `tokio` / any async runtime — same posture as ADR 0024.
- `tantivy` / any search index — search is per-document linear scan
  to keep the extraction footprint small. Revisit if a future
  story needs cross-session search inside the viewer.
- `nucleo` / fuzzy matcher — substring search only in v1,
  matching ADR 0024's stance.
- `indexmap`, `toml`, `toml_edit` — config concerns belong outside
  the viewer.

`docs/transcript-viewer-deps.md` carries this table as the
maintained source of truth. The list grows by ADR; never silently.

### Out-of-scope for v1

- Search across multiple sessions. The viewer is one-session-at-
  a-time.
- Cross-session lineage navigation. Conspectus's graph stays
  responsible for that; the viewer takes the resolved session
  as input.
- Resume / fork actions. Those continue to flow through
  `P8-011` resume and the harness-specific binaries.
- Aider transcripts (per ADR 0019 and `H-PREVIEW-005`
  deferral) — the viewer surfaces "transcript not supported"
  for aider rows and points operators at `claude-history`'s
  positional file path or the external launch if installed.

### Initial scroll position

The viewer opens with the **last turn focused** (analogue of the
operator's stated preference for claude-history). This is the
ADR-frozen contract; `G` / `End` jumps back to the end after
scrolling, `g` / `Home` jumps to the start. Initial focus on
the most recent turn matches the "what was that session about?"
read of the inline preview.

### Privacy / `--no-live-preview`

The same posture established for the inline preview (ADRs 0023 and
0051) carries through: transcript text never leaves the local
process. `--no-live-preview` disables the viewer entirely — the
`T` keybind reports "viewer disabled by --no-live-preview".
Operators who want zero-touch can also disable the harness
adapter via `CONSPECTUS_*_STATE` envs.

## Consequences

- The conspectus distribution no longer requires patched recall.
  `pkgs/recall/` in the Nix overlay can be reverted to a thin
  prebuilt wrapper (or removed) once the native viewer ships.
  Until then both paths coexist.
- The `T` keybind's resolver gains a "native viewer" backend that
  always wins over external backends, unless the user has
  opted into external via `[viewers.<harness>]` config
  (`H-TRANSCRIPT-013`).
- Conspectus owns more rendered UI surface than the original
  product surface contemplated. The data-model-first guardrail
  still holds: the viewer is a *presentation* of an existing data
  surface (session transcripts that conspectus already reads to
  populate `last_message_preview`), not a new model.
- The viewer is extractable: the `src/viewer/` boundary is
  policed by import rules + the dep-surface manifest. Lifting
  it later requires only (a) creating a new workspace member
  crate, (b) copying the module + the listed deps, (c) writing
  a `main.rs` that consumes `SessionLocator` from clap. No
  refactor inside conspectus.
- Recall debt rolls off over time as the recall backend becomes
  escape-hatch only. `H-TRANSCRIPT-014` and `H-TRANSCRIPT-015`
  are reclassified as "won't fix on the conspectus side" —
  operators who want recall coverage upstream submit patches
  there directly.
- Code volume grows by roughly 1.0-1.5k LOC for the viewer
  module (parser, widget, state, render). The marginal cost
  over the already-planned inline preview
  (`H-TRANSCRIPT-009`/`H-TRANSCRIPT-010`) is roughly 50% —
  full-screen layout, scroll, search, jump-to-end, and the
  conversion of the recent-history adapter into a
  full-transcript adapter.

## Alternatives Considered

- **Stay with external viewers, pay all the recall debt.** ~300-450
  LOC of fork patches plus ongoing upstream-rebase tax with no
  guarantee the harder pieces land upstream. Rejected — the
  marginal cost of the native viewer is roughly the same and has
  no recurring overhead.
- **Drop multi-harness coverage; ship claude-history only.**
  Honest scope reduction but loses the multi-harness story the
  product targets. Rejected.
- **Build the viewer as a workspace member crate from day one.**
  Forces the boundary structurally rather than by convention.
  Rejected for v1 because the project is currently a single
  crate; introducing a workspace adds CI/build ceremony out of
  proportion to the boundary value. The dep-surface manifest +
  import-rule discipline cover the boundary at lower cost, and
  extraction remains mechanical when it becomes worth it.
- **Reuse the H-TRANSCRIPT-009 inline preview widget as-is for
  full-screen.** Considered. The inline widget is sized for a
  right-pane height and renders without an active focus
  cursor; growing it into a full-screen modal needs the
  scroll/search/focus infrastructure regardless. Cleanest path
  is one widget that takes a layout target (inline vs.
  full-screen) and shares the rendering primitives.
- **Adopt `cass` (`coding_agent_session_search`) as the external
  default.** Closer to multi-harness coverage than recall, but
  `cass view` is a single-line lookup (not a scrollable viewer)
  and the license carries a non-standard OpenAI/Anthropic rider.
  Rejected for now; revisit if rider is dropped.

## Open Questions Answered

- The viewer ships in-tree as the default. External launches
  stay as escape hatch (ADR 0019, repurposed).
- Parsers are duplicated from `src/discovery/harness/*` for now;
  the discovery layer migrates to consume viewer parsers over
  time as a follow-up refactor, not blocking v1.
- The viewer's dep surface is enumerated above and tracked in
  `docs/transcript-viewer-deps.md`. New deps require an ADR
  amendment or follow-on ADR.
- Initial scroll lands on the last turn (most recent message
  focused) by default.
- Aider stays deferred.
- Search inside the viewer is substring-only in v1; library
  search index (tantivy etc.) deferred behind a future ADR.
