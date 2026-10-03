---
id: CSP-343
title: Styling + spacing pass
status: Done
assignee: []
created_date: '2026-06-03 00:29'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-340
ordinal: 216000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: CSP-338/CSP-340 ship a functional but
  visually-minimal modal. Operator feedback from kicking the
  tires: spacing is off (tool-output line numbers bump
  directly into content), turn separation is too subtle,
  role headers don't carry enough visual weight, and tool /
  thinking blocks need more chrome to read as folded-by-
  default content. Pass over:
  - **Turn-level chrome**: visible separator between turns
    (rule or extra spacing), accent color per role
    (`you` vs `assistant`), optional timestamp suffix on the
    header line (already in the model, not rendered yet).
  - **Compaction summary**: full-width rule + a distinct
    banner color so it reads as a structural marker rather
    than another role header.
  - **Body styling for `Message`/`CompactionSummary`**:
    consistent left indent so role headers visually own the
    body that follows; line wrapping (`Wrap { trim: false }`)
    to avoid mid-word breaks; soft padding on either side so
    Markdown bold/italic ranges don't crowd terminal edges.
  - **Tool blocks**: bracketed framing (e.g. `┌─ tool call:
    <name> ─` / `└─ tool result ─`) so call+result reads as
    a paired unit when both are visible. Argument JSON
    should be pretty-printed (or at least line-broken on
    commas) rather than one-line. Tool output rendering
    needs gutter handling — line numbers, lead-in prefixes
    etc. should sit in a fixed-width gutter that doesn't
    collide with content.
  - **Thinking blocks**: prefix with a distinguishable
    marker (e.g. dim italic `~ thinking ~`) so they're
    obviously not assistant-output prose.
  - **Code block styling inside `tui-markdown`**: re-evaluate
    the `highlight-code` feature decision from ADR 0051.
    The widget renders Markdown via `tui-markdown` with
    `default-features = false`; turning `highlight-code` on
    adds `syntect` (~MB), which ADR 0051 explicitly
    rejected for v1. Revisit if operators report that
    monochrome code blocks hurt scannability; otherwise add
    a soft fence indent + dim background as cheap
    substitutes.
  - **Theme integration**: per ADR 0052's `theme.rs` carve-
    out, route every color decision through the
    `crate::tui::theme::Theme` re-export so users can
    override via `[tui.theme]` config (ADR 0032).
- Tests: refresh insta snapshots for each turn kind (single
  Message, paired ToolUse+ToolResult, Thinking block,
  CompactionSummary banner). Add a snapshot for a narrow
  (40×24) terminal so the gutter / wrap behavior regresses
  visibly. The existing 4 widget snapshots stay as the
  baseline coverage; this story replaces them.
- Out of scope: search highlighting (covered by
  `CSP-339`), per-message expand/collapse
  (likely a separate story once tool-block framing is in),
  mouse bindings (`CSP-344`), in-viewer fork /
  child navigation (`CSP-345`).
- Blockers: `CSP-340`. Pairs naturally with
  `CSP-339` since both touch the renderer.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rewrote `src/viewer/render.rs` around a
`claude-history`-inspired gutter-and-chip layout:
right-aligned colored chips (`you`, `assistant`, `Thinking`,
`Tool`, `↳ Result`, `compact`) in a fixed `GUTTER_WIDTH=10`
column, separated from body by ` │ ` dim rule, with body
flowing to the right. Continuation lines (multi-line bodies
+ wrapped long lines) carry a blank gutter + repeated rule
so the body's left edge stays constant. Hand-rolled
word-wrap helper handles whitespace splits + hard-break for
oversized tokens; styled-line wrap preserves span styles
across line breaks for Markdown content. Chip styling
routed through the `crate::tui::theme::Theme` re-export
(ADR 0052's carve-out) so `[tui.theme]` config applies.
`src/viewer/widget.rs` updated: header refreshed to
`harness-chip · cwd · N turns` with width-aware degrade;
footer refreshed to `[ pos/total ] · tools·on/off ·
think·on/off · q close · …` with the long hint truncating
last. ADR 0051 amended to turn the `highlight-code`
feature **on** (re-decision rationale + counterfactual
both preserved); `syntect` added to
`ALLOWED_EXTERNAL_DEPS` and
`docs/transcript-viewer-deps.md`. Refreshed 4 prior
widget snapshots + added 2 new ones (tools-visible chip
pair, narrow-terminal 40-col wrap). 13 render-side tests
+ 9 widget-side tests; 1231 nextest green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-011`
