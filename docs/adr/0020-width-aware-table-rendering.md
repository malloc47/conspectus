# ADR 0020: Width-Aware Table Rendering

## Status

Accepted.

## Context

`conspectus session` is Conspectus's primary human-facing surface. The current
renderer in `src/output/table.rs` (~45 LOC of formatting) hand-rolls column
alignment with a fixed two-space padder and never measures the terminal width.
Several cells routinely exceed any reasonable terminal width:

- `CWD` — full absolute paths to checkouts, often 60-120 columns on their own.
- `AGENT` — `<harness_key>:<title-or-key>` where titles are free-form.
- `MUX` — `<backend>:<session>` where session names can be long.
- `PR` — `<owner>/<repo>#<n> (state)` is concise, but the column right-aligns
  against everything to its left.
- `LINEAGE` — short ids today, but trends longer once unresolved labels appear.

In an 80- or 120-column terminal these cells force wrapping or word-soup
overflow. The table is effectively unusable without `| less -S` or terminal
zoom.

The hardening backlog (`H-TBL-*`) groups the fix into five stories: an ADR
(this one), short row identifiers, width-aware truncation, an opt-in card
layout, and node-show acceptance of short ids. This ADR decides how the
renderer itself is implemented; the downstream stories assume the decision
landed here.

Requirements per the backlog and CLAUDE.md:

- Minimal dependency surface; new dependencies require an ADR.
- Deterministic byte-for-byte output across runs and platforms so the
  existing `insta`-style snapshot tests stay stable.
- First-class truncation with ellipsis (not just wrapping); wrapping multiplies
  the row count in a way that breaks the session-table mental model.
- A clean path to a multi-line/card layout in `CSP-129` without reshaping
  the renderer module again.
- Reusable from a future `graph --format text` projection (`CSP-093`) and
  any other text surfaces, so the renderer must live in a shared `output`
  module rather than inside one command.
- Unicode-aware column widths (the table includes CJK paths, emoji-laden PR
  titles in some forges, and combining marks in harness session titles).
- Correct behavior on TTY vs pipe: detected width on a TTY, untruncated when
  piped, with explicit overrides for reproducible captures.

## Candidates Considered

### `comfy-table` (v7.2.2, January 2026)

- Mature, widely used. Supports `ContentArrangement::Dynamic` and
  `DynamicFullWidth` for width-aware layout, with width detected via
  `crossterm::terminal::size`.
- Behavior on overflow is to **wrap** rather than truncate. Truncation with
  ellipsis is not in the public API; achieving it requires pre-truncating
  cells before handing them to comfy-table.
- No native rotated / extended-row layout for `CSP-129`.
- Upstream is in feature freeze (the maintainer is searching for a successor),
  which is a concern for evolving features we are about to add.
- Dependency footprint with the default `tty` feature: `crossterm` plus its
  platform glue. The `tty` feature can be disabled, in which case the
  consumer must call `Table::set_width` explicitly and the dep set shrinks
  considerably, but width detection then becomes our problem anyway.

### `tabled` (v0.x, active)

- Actively maintained, broad feature set. Native
  `Width::truncate(N).suffix("…")` modifier is an exact fit for `CSP-128`.
- Native `ExtendedTable` / `Rotate` covers `CSP-129` directly.
- Default features include `std` and `derive`. `derive` is unnecessary
  because we build rows at runtime; disabling it keeps the surface
  contained. `ansi` and `macros` are non-default.
- Tradeoffs:
  - Transitive dep tree (`papergrid` and friends) is larger than the other
    finalists, even with default features off.
  - The crate has historically reshaped its public API across minor
    versions, which is friction for stable snapshot tests.
  - Width detection is not bundled; consumers still need
    `terminal_size` (or equivalent) to feed an explicit width.

### `cli-table` (v0.5.0, March 2025)

- Maintained but lacks documented truncation, width-aware layout, and
  vertical / card layouts. Adoption would not address any of the
  H-TBL-* requirements.

### `prettytable-rs` (v0.10.0, December 2022)

- Last release more than three years old, 31 open issues and 19 pending
  PRs with no movement. No truncation, no width-aware layout. Effectively
  unmaintained for this use case.

### Roll our own minimal renderer

The existing `format_rows` in `src/output/table.rs` is ~45 LOC. The additions
needed to satisfy `CSP-127`..`CSP-130` are bounded:

- Width-aware truncation with ellipsis using `unicode-width` for column
  measurement: roughly 20-30 LOC.
- Terminal width detection on a TTY using the `terminal_size` crate
  (`rustix`-only on Linux/macOS, `windows-sys` on Windows): roughly 10 LOC.
- TTY-vs-pipe detection: existing `IsTerminal` from the standard library.
- A vertical/card layout for `CSP-129`: roughly 30-50 LOC.
- A renderer trait or a small set of public functions in `src/output/` so
  `CSP-093` and other surfaces can reuse the same primitives.

Dependency cost: two small crates.

- `unicode-width` 0.2.x — `#![no_std]`, no transitive deps.
- `terminal_size` 0.4.x — depends on `rustix` (Linux/macOS) or
  `windows-sys` (Windows). Both already pervasive in the Rust ecosystem
  and likely transitively present once any modern terminal-aware
  dependency lands.

Tradeoffs:

- More code under `src/output/` than adopting a library would require.
  Mitigated by the fact that the surface is well-bounded by the H-TBL-*
  stories and unlikely to grow beyond truncation, card layout, and
  optional separators.
- Unicode-aware truncation has edge cases (combining marks, double-width
  chars, ANSI escapes if we ever add color) that a battle-tested library
  handles for free. We avoid ANSI by keeping table output color-free; the
  remaining cases are handled by `unicode-width`.
- We own the byte-for-byte output, which is exactly what the snapshot
  tests need.

## Decision

Roll our own minimal width-aware renderer in `src/output/`, depending only on
`unicode-width` and `terminal_size`.

Concretely:

- Introduce a `src/output/table/` module (the current `src/output/table.rs`
  becomes `src/output/table/mod.rs`) with a small renderer that takes a
  `Vec<Vec<String>>` plus a `RenderOptions { width: Option<usize>,
  layout: Layout }` and emits a `String`.
- `Layout::Columnar` (default) renders the existing two-space-padded table
  with width-aware truncation; cells that exceed the per-column budget are
  truncated with a `…` suffix.
- `Layout::Card` (added in `CSP-129`) renders one column per line per row
  with a blank line between rows.
- Width is taken from `RenderOptions.width` when set, otherwise from
  `terminal_size::terminal_size()` when stdout is a TTY, otherwise
  unbounded (so pipes stay wide). The CLI surfaces `--wide` and
  `--width <N>` flags in `CSP-128`.
- Per-cell display widths use `unicode_width::UnicodeWidthStr::width`. The
  ASCII fast path stays inline; non-ASCII strings go through
  `unicode-width`.
- The renderer never emits ANSI escapes. Color, if added later, gets its
  own ADR.
- The snapshot fixtures continue to pin a fixed width via the new
  `RenderOptions.width` field, keeping `cargo test` output deterministic
  regardless of the developer's terminal width.

Both dependencies are runtime dependencies; this ADR records their
addition per CLAUDE.md's dependency policy.

## Consequences

- The renderer in `src/output/` becomes the shared text-output primitive for
  `conspectus session`, `CSP-093` (`graph --format text`), and any future
  text surface, without locking the project into a third-party renderer's
  shape.
- Snapshot tests stay byte-for-byte stable because the renderer is in-tree
  and only changes when this project changes it.
- The default `conspectus session` output becomes usable in 80- and
  120-column terminals once `CSP-128` lands on this renderer.
- Two small runtime dependencies are added; the crate's external surface
  grows by `unicode-width` and `terminal_size` plus their transitive
  platform-specific deps (`rustix` on Linux/macOS, `windows-sys` on
  Windows).
- Future TUI / color work will need a follow-up ADR; the renderer in this
  ADR explicitly stays plain text.
- If a downstream consumer (e.g. an Atelier embed) needs a richer renderer,
  the `output` module's public API gives them a seam without depending on
  the project's CLI behavior.

## Alternatives Considered

- **Adopt `tabled`.** Closest off-the-shelf match. Rejected because the
  default-feature surface is wider than needed, snapshot stability across
  minor versions is a recurring concern, and the in-house alternative is
  small enough that the avoided code does not justify the dependency.
  Reopen if the in-house renderer grows past ~300 LOC of formatting code
  or if we need an ANSI/color story that proves messy to hand-roll.
- **Adopt `comfy-table`.** Has dynamic width but wraps rather than
  truncates, has no vertical layout, and is in upstream feature freeze.
  Rejected primarily on the freeze risk and the missing truncation API.
- **Adopt `cli-table` or `prettytable-rs`.** Rejected for missing
  width-aware truncation and (for `prettytable-rs`) inactivity.
- **Pre-truncate cells in the existing renderer without adding any
  dependency.** Rejected because correct Unicode-width measurement
  requires `unicode-width` and ad-hoc byte/char counts will misalign on
  CJK paths and emoji-bearing PR titles.

## Open Questions Answered

- Truncation, not wrapping, is the default overflow strategy. Wrapping is
  not added by this ADR; if a future consumer needs it, the renderer can
  grow a `Layout::Wrapped` mode without disturbing existing behavior.
- The renderer lives in `src/output/`, not adjacent to a CLI command, so
  every text surface (session table, graph text projection, future
  diagnostics) shares it.
- TTY detection uses the standard library's `IsTerminal`; width detection
  uses `terminal_size`. `--wide` and `--width <N>` flags belong to
  `CSP-128`, not this ADR.
- Color is out of scope. The renderer stays ANSI-free until a follow-up
  ADR decides otherwise.
