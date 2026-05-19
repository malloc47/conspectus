# ADR 0022: Terminal Color And Styling For Table Output

## Status

Accepted.

## Context

`conspectus table <ROWS>` packs information-dense rows (provenance and
confidence indicators, PR state, declared-link status, fork lineage,
session ids) into compact text. Users have asked for color/style to
make these cells faster to scan. Color also helps the ambiguity
marker (`*`) and the dash placeholder (`—`) stand out from regular
content. The renderer is plain text today; adding color requires a
decision about:

- Which library backs the ANSI primitives.
- How `--color {auto|always|never}` resolves against environment
  conventions (`NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`,
  isatty).
- How the width-aware truncation cooperates with ANSI escape sequences
  that have non-zero byte length but zero display width.
- How existing insta snapshots and CLI integration tests stay stable
  (they should not silently gain ANSI codes).

The pager (H-TBL-013) already passes ANSI through (`less -R`).

## Decision

### Library

Use [`anstyle`] for the ANSI style primitives.

- `anstyle` 1.0 is already a transitive dependency via `clap` (clap
  uses it for its own colored help/error output), so adding it as a
  direct dependency costs zero new crates. Declaring it directly
  makes the intent explicit and locks the version against accidental
  drift from clap's upgrades.
- It's a small, no-allocation primitives crate (`Style`, `Color`,
  `Effects`) maintained by the rust-cli team. No global state.
- The ecosystem (clap, cargo's own colored output, gitoxide, …) has
  converged on `anstyle` as the lingua franca for color types, so
  using it keeps Conspectus's renderer interoperable if we ever want
  to plug in an `anstream::AutoStream` later for richer stream
  handling.

We do **not** add `anstream`, `colored`, `owo-colors`, or `termcolor`.
`anstream`'s `AutoStream` would automate strip-on-non-tty, but our
CLI already decides `--color=auto` against isatty/NO_COLOR before
calling the renderer, so the stream wrapping would be redundant.
`owo-colors` and `termcolor` would add competing styling APIs without
benefit for the surface we render.

[`anstyle`]: https://docs.rs/anstyle

### `--color` resolution

Each table-rendering subcommand (`conspectus table <ROWS>`,
`conspectus columns <ROWS>`, `conspectus node show <id>`) gains a
`--color {auto|always|never}` flag. The renderer's `RenderOptions`
gains a `color: bool` field. The CLI resolves the flag against the
environment to a boolean and passes it through.

Resolution rules, in priority order:

1. `--color=never` ⇒ false.
2. `--color=always` ⇒ true.
3. `NO_COLOR` env set to any non-empty value ⇒ false. Per
   <https://no-color.org>, this MUST override `--color=auto`. We do
   **not** let it override `--color=always`, matching the
   well-established cargo/git/ripgrep convention that explicit
   user-supplied flags win.
4. `CLICOLOR_FORCE` env set to a non-zero value ⇒ true. BSD-style
   override; treated the same as `--color=always` for resolution
   purposes.
5. `TERM=dumb` ⇒ false. Conservative; terminals that announce
   themselves as dumb often cannot display ANSI sequences.
6. `CLICOLOR=0` ⇒ false. BSD-style opt-out.
7. Otherwise (default `--color=auto`): true iff stdout is a TTY.

Pipes inherit `auto`'s "stdout is not a TTY → no color" behavior, so
`conspectus table sessions | grep` stays grep-friendly and existing
CLI tests (whose captured stdout is non-TTY) keep emitting plain
text.

### Renderer integration

`agent_cell` / `mux_cell` / `union_cell` / `pr_cell` / `fork_cell` are
extended to return a `Cell { text: String, style: anstyle::Style }`
instead of bare `String`. The width-aware columnar layout measures
`cell.text` only (ignoring zero-width ANSI), pads with uncolored
spaces, and emits the styled ANSI envelope around `cell.text` only
when `RenderOptions.color` is true. When `color` is false the styles
are dropped and the output is bit-identical to today's renderer, so
every existing insta snapshot stays stable without re-acceptance.

Truncation runs on `cell.text` before styling, so `…` inherits the
cell's style and the truncated output stays narrow enough for the
width budget.

### Initial palette

Conservative, semantic, low-noise. Designed to read on both light and
dark backgrounds. Concrete v1 mapping (subject to iteration in a
follow-up story):

| Cell content                       | Style                       |
| ---------------------------------- | --------------------------- |
| Header row (column names)          | **bold**                    |
| Dash placeholder (`—`)             | dim                         |
| Short ID column value              | dim                         |
| Indicator `LD`/`GD` (declared)     | green                       |
| Indicator `SD` (strong discovered) | cyan                        |
| Indicator `D`  (discovered)        | default                     |
| Indicator `C`  (convention)        | dim                         |
| Indicator `$`  (cached)            | dim                         |
| Indicator confidence `H`/`M`/`L`   | inherits indicator color    |
| Ambiguity marker `*`               | yellow                      |
| Lineage `?` prefix (unresolved)    | yellow                      |
| PR state `open`                    | green                       |
| PR state `closed`                  | red                         |
| PR state `merged`                  | magenta                     |
| PR state `draft` cell              | yellow                      |
| Declared state `declared`          | green                       |
| Declared state `ignored`           | dim                         |
| Declared state `overridden`        | yellow                      |
| Card-layout key (`KEY:` prefix)    | **bold**                    |

These are 16-color/256-color-safe choices using `anstyle`'s
`AnsiColor` palette; we do not require truecolor support.

## Consequences

- Each affected subcommand gains `--color {auto|always|never}`.
- Renderer API gains `RenderOptions.color: bool` and a small `Cell`
  type for styled output. Default of `false` keeps every snapshot
  byte-stable.
- `anstyle = "1"` becomes a direct runtime dependency (no new
  transitive crates since clap already pulls it).
- `docs/operations.md` documents the flag and the env precedence.
- Future palette refinements (per-harness colors, per-provider
  colors, configurable themes) are unblocked once this baseline
  lands; we expect to iterate.
- The width-aware budget logic stays unchanged because cells are
  measured on the unstyled `text`.

## Alternatives Considered

- **`owo-colors`.** Has built-in TTY/NO_COLOR/CI detection through
  the `supports-color` feature. Rejected because (a) we'd add new
  deps when `anstyle` already covers our needs, and (b) `owo-colors`
  extension-trait API is convenient but produces `Display` wrappers,
  not the structured `Style` value the renderer needs to attach to
  pre-computed cells.
- **`termcolor`.** Mature and well-tested (cargo/rustc/ripgrep
  history). Rejected because it adds a parallel styling API to the
  `anstyle` already in our build, and its `WriteColor` trait is
  oriented at incremental stream writes rather than the
  "build-the-table-then-emit" shape Conspectus uses.
- **`anstream::AutoStream` for output.** Would automate
  strip-on-non-tty. Rejected because our `--color=auto` resolution
  already decides at flag time; layering `AutoStream` on top would
  duplicate the decision.
- **Hand-rolled ANSI escape constants.** Tempting (~30 LOC), but
  loses interop with `anstyle::Style` consumers (e.g. future
  `anstream` adoption, ports to alternate terminals) for very little
  saved code. Rejected.
- **Color the whole output stream rather than per-cell.** Would
  preclude the width-aware truncation refactor needed to handle
  ANSI's zero display width. Rejected.

## Open Questions Answered

- The `Cell` type lives inside `src/output/table.rs`; it is not
  exposed in `conspectus::api` until a consumer asks. Until then,
  external callers continue to receive a plain `String` from
  `render`/`render_with`.
- Per-harness or per-provider color customization is deferred to a
  follow-up story. v1 ships a fixed palette.
- A future TUI surface (if any) would build on the same `Style`
  values, so the renderer's color choice is the "canonical" source.
