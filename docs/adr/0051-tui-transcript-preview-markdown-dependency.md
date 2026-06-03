# ADR 0051: TUI Transcript Preview Markdown Rendering Dependency

## Status

Accepted. Amended under `H-VIEWER-NATIVE-011` (styling pass) to
turn the `highlight-code` feature **on** after operator feedback
that monochrome fenced code blocks hurt scannability. The
original v1 decision to keep `highlight-code` off is preserved
in the §"Why turn off `highlight-code`" section as the
counterfactual; see §"Amendment: re-enable `highlight-code`
for the native viewer" at the bottom for the revised cost /
benefit.

## Context

The transcript-preview workstream (`H-TRANSCRIPT-*`, ADR 0019 follow-on)
replaces the single-line `last_message_preview` rendered in the TUI's
right panel for un-muxed `AgentSession` rows with a styled multi-turn
preview that fills the available right-panel height. Each turn is a
user or assistant message body whose content is, for every harness
the v1 product targets (Claude Code, Codex, OpenCode), Markdown — the
same Markdown the user sees in the source CLI.

Rendering that content as raw text loses the cues operators rely on
to recognize what an agent is doing in a glance: headings, bold/italic
emphasis, list structure, fenced code, inline `code`, and link
formatting. ADR 0025 established the parallel precedent for muxed
panes: a captured `tmux capture-pane -e` stream is parsed by
`ansi-to-tui` into styled `ratatui::text::Text` rather than rendered
as a monochrome wall. The transcript preview is the un-muxed analogue
and needs an analogous rendering path.

ADR 0024 reserved the right to pull in new TUI crates only after a
follow-on ADR. `H-TRANSCRIPT-001` asks for this ADR before
`H-TRANSCRIPT-008` adds the dependency.

Three candidate rendering paths surveyed:

1. **`tui-markdown 0.3.7`** (`joshka/tui-markdown`,
   MIT OR Apache-2.0). Public API is
   `tui_markdown::from_str(&str) -> ratatui::text::Text`. Built on
   `ratatui-core` and tracks ratatui 0.30. Default feature
   `highlight-code` pulls in `syntect 5.x` plus `ansi-to-tui 8.0.1`
   for fenced-code syntax highlighting; with `default-features =
   false` neither dep enters the graph. Active maintenance: 0.3.7
   released December 2025; ~300k downloads to date.
2. **`termimad 0.34.1`** (`Canop/termimad`, MIT). Mature Markdown
   renderer (5M+ downloads). Targets `crossterm` directly with its
   own draw loop and skin abstraction; explicitly "not a TUI
   framework". Has no native ratatui `Text` output, so adopting it
   would require either bridging termimad's own painter into a
   ratatui-managed area (fights the immediate-mode posture in
   ADR 0024) or re-rendering termimad output to ANSI and feeding
   it through `ansi-to-tui` (extra hop, lossy spans).
3. **Roll our own over `pulldown-cmark 0.13.x`** (MIT, the de-facto
   CommonMark parser, 100M+ downloads). The parser is event-stream
   only; producing styled `ratatui::text::Text` means hand-writing
   the event-to-span mapping, list nesting, soft/hard break
   handling, code-fence rendering, table layout, and link styling.
   That is exactly what `tui-markdown` already implements on top
   of `pulldown-cmark`.

Existing TUI dependencies relevant to the choice:

- `ratatui 0.30` (default-features = false, `crossterm` feature) —
  ADR 0024.
- `ansi-to-tui 8.0.1` (default-features = false) — ADR 0025. Used
  today only for muxed pane captures.
- No `syntect`, no `tokio`, no markdown crate.

## Decision

Adopt **`tui-markdown 0.3.7`** with **`default-features = false`** as a
direct dependency of the `conspectus` crate, scoped to the
`src/tui/` module. The `highlight-code` feature stays off in v1.

Use it from the inline transcript-preview widget added by
`H-TRANSCRIPT-009`. Each `TranscriptTurn` body is passed through
`tui_markdown::from_str` to produce a `ratatui::text::Text`; the
widget composes per-turn role headers (dimmed `you` / `assistant`
labels), the rendered body, and compaction-summary markers, then
hands the result to a `Paragraph` for layout, cropping, and scroll
behavior.

### Why turn off `highlight-code`

- The preview surface is the *recent* N turns, not a code-reading
  tool. Most preview content is conversational prose; when code
  blocks appear they are short snippets that read fine in the
  default `Style::reset()` plus a fence indent.
- `syntect 5.x` is the largest dep weight in the candidate graph
  (parser + bundled syntaxes / themes), and `highlight-code` also
  drags `ansi-to-tui` into the markdown path. We already use
  `ansi-to-tui` for muxed previews per ADR 0025, but the markdown
  renderer doing a syntect → ANSI → ansi-to-tui round-trip just to
  colorize a transcript fence is incidental complexity for the
  preview pane's value.
- ADR 0016 (single-binary distribution) and ADR 0024's narrow
  TUI dep policy both reward keeping the v1 graph minimal.
  Re-enabling `highlight-code` is reversible if operator feedback
  finds plain code blocks too hard to scan.

### Why not termimad

- termimad targets crossterm directly. ADR 0024 fixed an immediate-
  mode Ratatui posture with a pure `update` reducer and a
  buffer-snapshot test seam. Adopting a renderer that owns its own
  paint loop or that we'd shim through ANSI would erode that
  boundary for a worse rendered result than `tui-markdown` already
  produces.
- termimad's strengths (skins, templates, the question/`ask!`
  APIs) are for stand-alone CLI prompts and dashboards. None of
  them apply to a right-panel transcript preview.

### Why not roll-your-own on `pulldown-cmark`

- The 100–150-LOC estimate familiar from ADR 0025's SGR-parser
  rejection applies here too, only more aggressively: Markdown's
  edge cases (loose vs. tight lists, soft-break vs. hard-break,
  nested emphasis, link reference resolution, fenced vs. indented
  code) have long tails that `tui-markdown` already handles and
  tests against.
- The same maintenance argument from ADR 0025 holds: every
  CommonMark edge case we don't get to surfaces as an
  operator-visible glitch in the preview pane. Owning a markdown
  renderer in-tree is exactly the kind of incidental surface area
  the dependency-policy ADRs were written to discourage.
- `tui-markdown` is itself a thin layer over `pulldown-cmark` — we
  inherit the parser quality without owning the renderer.

### Integration shape

A new module `src/tui/transcript_preview.rs` (per
`H-TRANSCRIPT-009`) owns the widget. It depends on `tui-markdown`
only at the body-rendering call site; the rest of the widget
operates on `Vec<TranscriptTurn>` from the recent-history adapter
defined by `H-TRANSCRIPT-003`. The transcript-preview path is
distinct from the muxed-preview path:

- Muxed selections: `tmux capture-pane -e` →
  `ansi-to-tui::IntoText::into_text` → `Paragraph` (ADR 0025
  status quo).
- Un-muxed selections: harness recent-turns adapter →
  per-turn `tui_markdown::from_str` → composed `Text` →
  `Paragraph` (this ADR).

`--color=never` (ADR 0022) continues to flow through `RunConfig`.
When color is disabled, the widget bypasses `tui_markdown::from_str`
and renders each turn body as a single unstyled span, mirroring the
fallback ADR 0025 established for muxed previews. The single-line
`last_message_preview` fallback for `--no-live-preview` (carried by
`AgentSessionNode` per ADR 0023) does not pass through `tui-markdown`
either.

### License and supply-chain posture

`tui-markdown` is MIT OR Apache-2.0, matching the rest of the dep
graph. With `default-features = false` the transitive graph the
crate adds is small: it builds on `ratatui-core` and `pulldown-cmark`
(both already first-tier in the Rust ecosystem; `pulldown-cmark` is
MIT). `syntect`, `ansi-to-tui`, and their transitives stay out of the
markdown path. No new build-time tooling is required; `cargo build`
and `cargo clippy --all-targets --all-features -- -D warnings`
remain the gating checks.

## Consequences

- The un-muxed transcript preview gains styled headings, emphasis,
  lists, and fenced code in the right panel without expanding the
  TUI module's surface area to include a hand-written markdown
  renderer.
- The dep graph grows by `tui-markdown` plus its `pulldown-cmark`
  transitive. With `highlight-code` off, no `syntect` and no
  duplicated `ansi-to-tui` paths land in the graph.
- Snapshot tests over `transcript_preview` assert on styled `Text`
  contents at fixed terminal sizes (the existing `insta` +
  buffer-snapshot harness from ADR 0024). Plain-text assertions
  continue to work via `Text`'s flattened-string view, the same as
  ADR 0025 noted for muxed previews.
- `--color=never` continues to strip styling. The `RunConfig.color`
  flag gates `from_str` use, so the v1 color contract (ADR 0022)
  stays consistent.
- Re-enabling `highlight-code` later is a single-line `Cargo.toml`
  change with no model or API impact; a follow-up ADR is only
  needed if the syntect transitive grows the TUI dep posture in a
  way ADR 0016 / ADR 0024 would not already cover.

## Alternatives Considered

- **`termimad`.** Rejected because it targets `crossterm` directly
  with its own painter, has no ratatui-native `Text` output, and
  pursuing it would either fight the immediate-mode posture from
  ADR 0024 or require an ANSI round-trip that `tui-markdown` already
  avoids.
- **Roll-your-own renderer over `pulldown-cmark`.** Rejected on the
  same grounds ADR 0025 rejected an in-tree SGR parser: the long
  tail of edge cases (list looseness, link references, fenced vs.
  indented code) would surface as operator-visible glitches and the
  maintenance burden would offset any saved bytes.
- **Skip styling and render plain transcript text.** Rejected
  because the un-muxed selection is the dominant TUI surface for
  agent sessions, and operators flagged the equivalent missing
  styling on muxed panes during binary validation (the motivating
  context for ADR 0025).
- **`tui-markdown` with `highlight-code` enabled.** Rejected for v1
  to keep `syntect` out of the dep graph; the preview's value
  proposition is recent-conversation context, not code reading.
  Trivially reversible.
- **A native markdown layer inside a future Phase 7 server.**
  Rejected as the v1 path because the transcript-preview surface
  is a TUI render-time concern and Phase 7 transport boundaries
  carry pure data, not styled `Text`. Server-side rendering would
  also force one renderer's styling choices on every client.

## Open Questions Answered

- The chosen crate is `tui-markdown 0.3.7`, not `termimad` and not
  an in-tree roll-your-own.
- The `highlight-code` feature stays off in v1 (no `syntect`,
  no second `ansi-to-tui` path through the markdown layer).
- License (MIT OR Apache-2.0) and supply-chain posture
  (`pulldown-cmark` transitive only) match the rest of the crate.
- `--color=never` continues to bypass the markdown renderer
  (consistent with ADR 0025's muxed-preview color contract).
- Integration lives in `src/tui/transcript_preview.rs`
  (`H-TRANSCRIPT-009`). `H-TRANSCRIPT-008` adds the dep to
  `Cargo.toml` in isolation.

## Amendment: re-enable `highlight-code` for the native viewer

Operator feedback after `H-VIEWER-NATIVE-008` shipped the
native full-screen viewer: monochrome code blocks bury syntax
cues that operators read past prose to find — function names,
type annotations, keyword vs string vs comment colouring. The
v1 "skip syntax highlighting" trade matched the inline preview's
single-pane budget (short snippets, small surface area) but
underestimated the full-screen viewer's "scan a tool-result
diff" workflow.

Re-decision: enable `tui-markdown`'s `highlight-code` feature.
Concrete change is a one-line `Cargo.toml` edit:

```toml
tui-markdown = { version = "0.3", default-features = false, features = ["highlight-code"] }
```

`syntect` lands in the dependency graph (as `tui-markdown`'s
transitive). `ansi-to-tui` is already a direct dep per
ADR 0025, so no new direct surface. Compiled size grows by
the bundled syntect grammars and themes — single-digit MB.
`docs/transcript-viewer-deps.md` updated to add `syntect` to
the maintained allow-list.

The original v1 trade — that the inline preview is conversational
prose, not a code-reading tool — still holds for the inline-
preview surface, but the same code is now rendered inside the
full-screen viewer where reading code is exactly the workflow.
Sharing one renderer between the two surfaces is the right call
even if it pulls the inline preview along for the ride; the
alternative (two configurations of `tui-markdown`) doubles the
build surface for no gain.

`--color=never` continues to bypass the renderer per
ADR 0022 / ADR 0025; with colour off, syntax highlighting is
moot anyway.
