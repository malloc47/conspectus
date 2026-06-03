# Transcript Viewer Dependency Surface

This document is the **canonical, maintained allow-list** of
crates the in-tree transcript viewer (`src/viewer/`) is permitted
to depend on. It exists per ADR 0052 to keep the module
extractable as a standalone CLI tool later: lifting it should be a
mechanical "copy module + listed deps + write a clap wrapper"
move, not a refactor.

**Rules**:

- Every direct crate import inside `src/viewer/**` MUST appear in
  the table below. CI does not enforce this today; reviewers do.
- Adding a new entry requires updating both this document *and*
  the dep-surface docstring on `src/viewer/mod.rs`. Both must
  match.
- Removing an entry requires confirming no `src/viewer/**` file
  still imports it.
- The module MUST NOT import from `crate::model`, `crate::resolve`,
  `crate::query`, `crate::discovery`, or `crate::tui::*` outside
  the viewer subtree. (One known-tracked exception is the
  `crate::tui::theme` palette; see ADR 0052 for the carve-out
  rationale.)
- Versions reflect the *project's* current pin; the extracted
  crate will inherit them at extraction time and bump
  independently after.

## Direct dependencies (library surface)

| Crate | Project pin | Features used | Why the viewer needs it |
|---|---|---|---|
| `ratatui` | `0.30` | `crossterm` (`default-features = false`) | Buffer, widget primitives, immediate-mode rendering of the full-screen modal and the inline preview. ADR 0024. |
| `crossterm` | (implicit via ratatui) | terminal events | Terminal lifecycle when the extracted binary owns its own runtime. Becomes explicit on extraction. ADR 0024. |
| `tui-markdown` | `0.3` (per ADR 0051, amended for ADR 0051's `highlight-code` re-decision under H-VIEWER-NATIVE-011) | `default-features = false, features = ["highlight-code"]` | Per-turn message-body Markdown → `ratatui::text::Text`, with fenced code blocks syntax-highlighted via `syntect`. ADR 0051 (amended). |
| `syntect` | transitive via `tui-markdown[highlight-code]` | (default) | Syntax highlighter for fenced code blocks. Pulls in bundled grammar/theme data (single-digit MB compiled). Adopted under H-VIEWER-NATIVE-011 after operator feedback that the un-highlighted code was hard to scan. ADR 0051 amended. |
| `ansi-to-tui` | `8.0.1` | `default-features = false` | Render tool-output ANSI styling when the operator opts in (tool blocks are hidden by default). ADR 0025. |
| `comfy-table` | `7` | `default-features = false` | Markdown table rendering inside message bodies. `ContentArrangement::Dynamic` + `set_width(content_width)` gives column-aware wrap-to-fit. Adopted under H-VIEWER-NATIVE-017. ADR 0054. |
| `rusqlite` | `0.39` | `bundled`, `load_extension` | OpenCode session reader; SQLite is OpenCode's record store. ADR 0013. `bundled` keeps the extracted binary single-file. |
| `serde` | `1.0` | `derive` | JSONL record types for Claude Code + Codex parsers. |
| `serde_json` | `1.0` | — | JSONL line parsing. |
| `anyhow` | `1.0` | — | Errors at the parser/widget seam. |
| `thiserror` | `2.0` | — | Typed errors inside parser modules. |
| `chrono` | *new — not yet a project dep* | `default-features = false`, `clock`, `serde` (if turn timestamps are serialized) | Turn timestamps in TranscriptTurn / TranscriptDocument. Add when the first parser that reads timestamps lands. |
| `unicode-width` | `0.2` (project pin) | — | Wrapping and truncation for variable-width content (CJK, emoji) in the scroll/search layer. |

## Binary-only dependencies

These appear in the *extracted* crate's `[[bin]]` target (or in
the conspectus binary today when the viewer is launched). They
are NOT imported by the viewer library module itself.

| Crate | Pin | Where used |
|---|---|---|
| `clap` | `4.5` | Argument parsing for the extracted CLI (`#[cfg(feature = "bin")]` gate). |

## Explicitly excluded

These are notable crates we deliberately keep *out* of the viewer
surface. Each rejection has a rationale in ADR 0052.

| Crate | Reason |
|---|---|
| `tokio` / `async-std` / any async runtime | Same posture as ADR 0024. No streaming use case in v1 justifies it. |
| `tantivy` / search-index libraries | Substring search only inside the viewer in v1. Extraction footprint stays minimal. |
| `nucleo` / fuzzy matcher | ADR 0024 stance carries over: substring is enough until proven otherwise. |
| `indexmap` | Config-shaped; lives outside the viewer. |
| `toml` / `toml_edit` | Config-shaped; lives outside the viewer. |
| Conspectus-internal crates (`crate::model`, `crate::resolve`, etc.) | Would couple the viewer to conspectus's graph and break extraction. |

## Update procedure

1. Open an ADR amendment (or follow-on ADR) when the addition
   isn't already covered.
2. Update this table.
3. Update the dep-surface docstring on `src/viewer/mod.rs`.
4. Add the dep to `Cargo.toml`.
5. The reviewer confirms all four agree before merging.

This rhythm is the same as ADR 0024's "any new TUI dep needs a
follow-on ADR" rule, scoped to the viewer module specifically.
