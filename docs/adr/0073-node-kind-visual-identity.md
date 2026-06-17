# ADR 0073: Per-Node-Kind Visual Identity (Glyph + Color)

## Status

Accepted

## Context

The `GraphNode` / `NodeId` enum has nine variants — `Workspace`,
`Repo`, `Checkout`, `AgentSession`, `MuxSession`, `RuntimeProcess`,
`Branch`, `Fork`, `ForgePr` (`src/model/mod.rs:189`). The TUI today
carries strong visual identity for two of them:

- `AgentSession` rows render a colored harness pill (`[claude]`,
  `[codex]`, …) via `src/tui/widgets/badge.rs`, with hues from
  `theme.harness_*`.
- `MuxSession` rows render the binary `◉ / ◯` attachability chip
  established by ADR 0072, plus the ambiguity warning `⚠` ADR 0071
  promoted to the group level.

The other seven kinds have no visual hook. `Workspace`, `Repo`, and
`Checkout` appear as group rows that read as a bold path with a
disclosure `▶`/`▼` glyph — visually indistinguishable from each
other. `Branch`, `RuntimeProcess`, `Fork`, and `ForgePr` appear in
the detail pane and relationship explorer as a dim `[kind]` text
chip (`src/tui/ui.rs:1905`) — also indistinguishable from each
other. Operators scanning a dense sessions tree or the right-pane
explorer have no fast way to tell *what kind of thing* a row
represents before reading its label.

The `H-VIS-001..006` workstream (`docs/backlog.md:9351`) scoped a
per-node-kind glyph + color system but parked it behind this ADR.
H-UI-002 promoted the workstream to scheduled; the ADR is the gate.

Three coupled decisions need to settle together because they share
one `[tui.theme]` schema, one icon-lookup module, and one set of
column-width invariants:

1. **Glyph strategy** — Unicode geometric shapes (1-cell, terminal-
   safe), Nerd Font pictographs (font-dependent), emoji (width-
   inconsistent), or a mixed approach.
2. **Per-kind slate** — which glyph, which color, and which theme
   key each of the nine kinds gets.
3. **Placement and override schema** — where the glyph sits in a
   row, how indent and column budgets account for it, and how
   operators swap the slate.

ADR 0072's binary mux chip, ADR 0071's group-level ambiguity
warning, ADR 0032's `[tui.theme]` schema, and ADR 0033's detail-
pane section model all stay intact. This ADR adds an identity
prefix; it does not relocate or replace any existing badge.

## Decision

### 1. Glyph strategy — geometric default, opt-in Nerd Font override

The shipped default for every node-kind glyph is a 1-cell Unicode
geometric shape. No new font dependency, no terminal capability
detection, no `--no-color` regression.

Operators with Nerd-Font-patched terminals override individual
glyphs through a new `[tui.theme.icons]` table (see decision 4).
The default slate continues to work unchanged when the override
table is absent or partial.

The geometric default matches the vocabulary already in use
across the renderer (`◉ ◐ ◯ ▶ ▼ ★ ⚠`) — all 1-cell, all
terminal-safe. Mixing pictographic icons into the same row would
introduce inconsistent cell widths and break the column-budget
invariants the ADR 0020 width-aware renderer and
`harness_badge_width` (`src/tui/widgets/badge.rs:49`) maintain.

### 2. Per-kind slate

| Kind             | Glyph | Codepoint | Default color | Theme color key       |
|------------------|-------|-----------|---------------|-----------------------|
| `Workspace`      | `▦`   | U+25A6    | LightBlue     | `node_workspace`      |
| `Repo`           | `◆`   | U+25C6    | Blue          | `node_repo`           |
| `Checkout`       | `◇`   | U+25C7    | Cyan          | `node_checkout`       |
| `AgentSession`   | `●`   | U+25CF    | LightGreen    | `node_agent_session`  |
| `MuxSession`     | `▣`   | U+25A3    | Magenta       | `node_mux_session`    |
| `RuntimeProcess` | `⚙`   | U+2699    | DarkGray      | `node_runtime_process`|
| `Branch`         | `⎇`   | U+2387    | Green         | `node_branch`         |
| `Fork`           | `⑂`   | U+2442    | LightMagenta  | `node_fork`           |
| `ForgePr`        | `⇄`   | U+21C4    | *(reuses `pr_open/closed/merged/draft`)* | — |

Selection rationale:

- **No collision** with the in-use vocabulary (`◉ ◐ ◯` mux, `▶ ▼`
  disclosure, `★` resolver winner, `⚠` ambiguity, `📌` pins).
- **Width** is 1 cell for every codepoint in common monospaced
  fonts. `⚙` is written as the bare codepoint without the
  `VARIATION SELECTOR-16` (`U+FE0F`); the bare form renders
  1-cell on every terminal in the project's support matrix. `⑂`
  (OCR HOOK) is the closest Unicode glyph to a fork shape; the
  ASCII fallback `Y` is exposed via the icon override table for
  terminals where U+2442 renders as `▯`.
- **Color independence.** `AgentSession`'s `●` glyph carries an
  *independent* kind color (`node_agent_session`, default
  LightGreen), separate from the harness pill's harness color.
  Two signals stack: "this is an agent session" (kind glyph) and
  "the harness is claude" (pill color). Collapsing them onto the
  pill was considered and rejected (see Alternative A).
- **`ForgePr` reuses `pr_*`.** The four existing PR-state colors
  (`pr_open`, `pr_closed`, `pr_merged`, `pr_draft`) already carry
  the state distinction the operator cares about; the `⇄` glyph
  takes the color of whichever PR-state field applies to the row.
  No new color field for `ForgePr` kind identity.

### 3. Placement rule

The kind glyph sits between the disclosure glyph (if any) and any
existing kind-specific badge. One layout law per row type:

```
<indent> <disclosure>? <kind-glyph> <kind-specific-badges>* <label> <secondary…>
```

Concretely:

- **Group rows** (`Workspace`, `Repo`, `Checkout`):
  `  ▼ ▦ /path/to/workspace` (disclosure, glyph, label).
- **`AgentSession` row**:
  `    ● [claude] showcase-claude   1h ago   /path` (glyph in
  node-kind color, pill in harness color, label).
- **`MuxSession` row**:
  `    ▣ ◉ project   project:0   tmux` (kind glyph, then the
  ADR 0072 binary chip, then native id).
- **Detail-pane neighbor chip** (replaces `kind_chip_span`):
  `  ◆ project   member of` (glyph + label, glyph in node-kind
  color, label in current dim style).

The glyph occupies exactly one cell. Indent arithmetic and column
budgets in `rows/*` add one cell uniformly per row; the
width-aware renderer (ADR 0020) subtracts that cell from the
right-edge budget before allocating secondary columns. Disclosure
alignment is unaffected because the disclosure column still owns
its own cell to the left of the new glyph.

When a row has no disclosure (most leaf rows), the kind glyph
sits in the column the disclosure would have occupied — there is
no offset to maintain. When a row has a disclosure (group rows),
the glyph is the next cell to the right. The
`indent + disclosure_width + glyph_width` total is stable across
expanded and collapsed states because the disclosure column keeps
its cell either way (per the existing `▶`/`▼` rendering at
`src/tui/ui.rs:1918`).

### 4. Theme schema — colors + icon override table

Two additions to ADR 0032's `[tui.theme]` schema:

1. **Eight new color keys**, one per kind that owns a color
   (`ForgePr` does not — see decision 2). They follow the same
   grammar as every other color key:
   ```toml
   [tui.theme]
   node_workspace       = "bright_blue"
   node_repo            = "blue"
   node_checkout        = "cyan"
   node_agent_session   = "bright_green"
   node_mux_session     = "magenta"
   node_runtime_process = "dark_gray"
   node_branch          = "green"
   node_fork            = "bright_magenta"
   ```
   Defaults reproduce the table in decision 2. Validation,
   warnings, and parsing reuse the existing
   `Theme::set_color` / `known_keys` machinery (`src/tui/theme.rs:217`).

2. **A new `[tui.theme.icons]` sub-table** mapping each
   `NodeKind` to its glyph string. Defaults match the table in
   decision 2; unset keys keep the default:
   ```toml
   [tui.theme.icons]
   # Nerd Font opt-in (requires a patched terminal font).
   node_repo            = ""  # nf-cod-repo
   node_branch          = ""  # nf-fa-code_branch
   node_fork            = ""  # nf-cod-repo_forked
   node_forge_pr        = ""  # nf-cod-git_pull_request
   # ASCII fallback for terminals that mis-render U+2442:
   node_fork            = "Y"
   ```
   Validation:
   - Each value is a `String`; the parser computes
     `unicode_width` and refuses any glyph whose display width is
     not exactly 1 cell, with a per-key warning. This keeps the
     row layout law (decision 3) intact regardless of operator
     overrides.
   - Unknown keys produce a single warning per key, same posture
     as the existing flat-schema validation (ADR 0032 §4).
   - The table is optional; absent table = use defaults.

The sub-table is the only nested block under `[tui.theme]`.
Decision 4 of ADR 0032 deliberately kept the color schema flat;
the icons live in their own table because they share the
*key-set* with the color fields but have a different *value
grammar* (string glyph vs color spec), and the operator's mental
model — "the icon set" — is a natural unit to override or paste.

### 5. Lookup module — `src/tui/icons.rs`

A new `NodeKind` enum mirrors the nine `GraphNode` variants with
`From<&GraphNode>` and `From<&NodeId>` conversions, plus a
`NodeKind::display_label() -> &'static str` returning the stable
snake-case tag (`"agent_session"`, `"forge_pr"`, …) currently
produced by the duplicated `kind_label` functions in
`src/tui/detail.rs:358` and `src/tui/explorer.rs:831`. Those two
helpers are replaced by calls to `NodeKind`.

A `NodeKindStyle { glyph: &'static str, color: fn(&Theme) -> Color, width: usize }`
struct and a `node_kind_style(NodeKind) -> NodeKindStyle` lookup
serve every render site. `ForgePr` returns a sentinel
`color: |_| Color::Reset` because its color comes from the
PR-state field on the row, not from the kind; callers that render
a `ForgePr` glyph pick the color from `theme.pr_*` directly.

Operator overrides flow through the same struct via a
`node_kind_style_with(NodeKind, &Theme, &IconOverrides)` variant
that swaps the glyph string from the parsed `[tui.theme.icons]`
table. No call site reaches into the override map directly.

### 6. Non-TUI surface — same `NodeKind` tag, no glyph propagation

The machine-readable surface (JSON `--format json`, DOT, HTML) is
out of scope for this ADR's *rendering* decisions and stays under
the H-VIS-005 story. The shared lift this ADR delivers is the
stable `NodeKind::display_label()` tag that JSON and DOT consumers
already need; the glyph and color *are* a TUI rendering concern.
The DOT renderer may opt into the node-kind color as a fill or
font color in a follow-up, but no machine-readable output relies on
the glyph string from `[tui.theme.icons]`.

### 7. Accessibility

The slate satisfies three accessibility constraints by
construction:

- **Glyph-only legibility.** Every kind has a distinct *shape*,
  not merely a distinct hue. `NO_COLOR` / `--color never` strips
  the color but leaves the glyph; a snapshot variant covering
  this case lands under H-VIS-006.
- **Color-blind safety.** The default palette deliberately mixes
  shape families (filled vs hollow, geometric vs technical) so
  the worst-case "all glyphs render in the same hue" reading is
  still distinguishable. No two glyphs share both shape and color
  family (e.g. `◆`/`◇` differ in fill even before color, `●`
  is the only round shape, `⚙`/`⎇`/`⑂` are technical, `⇄` is
  bidirectional).
- **Terminal capability.** All defaults are codepoints in the
  Unicode Geometric Shapes / Miscellaneous Technical /
  Miscellaneous Symbols / Arrows blocks, present in every
  major monospaced font (DejaVu, IBM Plex Mono, Iosevka, JetBrains
  Mono, SF Mono, Cascadia Code, the default `xterm`/`linux`
  terminal fonts).

## Consequences

- **Every row in every view gains one cell** for the kind glyph.
  Column budgets in `rows/*` shrink the secondary path / preview
  columns by one cell uniformly; the change is uniform across
  views so no per-view layout law changes.
- **The duplicated `kind_label` helpers** in
  `src/tui/detail.rs:358` and `src/tui/explorer.rs:831` collapse
  into one `NodeKind::display_label()` call site. The detail
  pane's `kind_chip_span` (`src/tui/ui.rs:1905`) becomes a
  colored glyph plus a dim label rather than a bracketed text
  tag.
- **AgentSession rows carry two independent signals** (kind glyph
  color + harness pill color). Operators reading "claude session"
  by the magenta pill keep that signal; new operators have the
  uniform `●` to anchor "this is an agent session" before
  learning the harness vocabulary.
- **`[tui.theme.icons]` is the first nested table** under
  `[tui.theme]`. ADR 0032's flat-schema decision is preserved for
  the color keys; the icons block is a deliberate exception
  because its value grammar differs.
- **Snapshot churn is wide but shallow.** Every left-pane row
  snapshot and every detail-pane snapshot picks up one prefix
  cell. The diffs are mechanical and reviewable per view.
- **Nerd Font users can adopt pictographic icons** without code
  changes by pasting a `[tui.theme.icons]` block. The default
  slate keeps working for terminals without Nerd Fonts (SSH
  sessions, CI snapshots, the project's own dev shell when the
  operator's terminal does not load a patched font).
- **`ForgePr`'s color reuse means PR rows do not gain a new color
  key.** Operators who want a kind-color distinct from PR-state
  hue can override the `ForgePr` glyph in `[tui.theme.icons]` but
  not the color; if a real use case appears, a follow-up ADR can
  add `node_forge_pr` without breaking this one.
- **`H-UI-005` (resolved-vs-candidate)** can use the new
  per-kind color fields as a starting point for its own
  `EdgeStateLabel` chip palette, keeping a single source of
  truth for "node-related color" in the theme.
- **`H-UI-003` (detail-pane flatten)** lands after this so the
  related-entities rows already have the per-kind glyph; the
  flatten can lean on glyph identity instead of column position
  for direction cues.

## Alternatives Considered

### A. Collapse the AgentSession kind glyph onto the harness pill

Replace the harness pill with a `●` glyph colored by harness
(magenta `●` for claude, cyan `●` for codex, etc.) and drop the
pill text. Rejected: the pill text is a learning aid — new
operators read `[claude]` before they have memorized that magenta
means claude. The two-signal layout costs one cell per row but
keeps the readable label.

### B. Nerd Font as the default

Ship `nf-cod-repo` and friends as the default slate, fall back to
geometric on parse failure. Rejected: silently degrading to `▯`
on terminals without a patched font would break snapshot tests
and confuse operators who do not realize their font is the
problem. The opt-in posture surfaces the requirement explicitly
in `[tui.theme.icons]`.

### C. Emoji slate

Use `📂 📁 🤖 🖥️ ⚙️ 🌿 🍴 🔀` as the default. Rejected: cell
widths vary by terminal and font (1-cell on some VT100-style
emulators, 2-cell on `kitty`/`iTerm`), which would break the
column-budget invariants the width-aware renderer maintains.
Variation-selector-16 (`U+FE0F`) appendage is required for
emoji-style rendering of several of these codepoints and makes
the actual glyph the operator sees depend on terminal-specific
heuristics.

### D. ASCII-only default with Unicode opt-in

Ship `* o # & + B Y > %` as the default and gate Unicode behind
the icon table. Rejected: the existing TUI vocabulary
(`◉ ◐ ◯ ▶ ▼ ★ ⚠ 📌`) already uses Unicode glyphs that ASCII
operators tolerate today; demoting the kind glyphs to ASCII
would be the only ASCII surface in the renderer, an inconsistent
posture without a corresponding policy decision (and one this
ADR explicitly does not make).

### E. Per-row vs per-cell glyph rendering

Render the glyph in a dedicated logical column (parallel to the
mux-state chip column) instead of as a prefix cell on the row.
Rejected: per-view column-count would grow, and the glyph reads
more clearly as part of the row's identity than as a peer to
state chips. Group rows in particular do not have a "mux state"
column to align against.

### F. Detect Nerd Font availability at runtime

Probe the terminal for a Nerd Font glyph (e.g. test if ``
renders at width 1) and auto-swap the slate. Rejected: detection
via terminal queries is unreliable across multiplexers and SSH
(ADR 0032 §6 rejected the same approach for light/dark
detection); the operator-opt-in posture is consistent with that
prior decision.

### G. Drop `ForgePr` glyph entirely

Render PR rows with only the `pr_*`-colored label, no leading
glyph. Rejected: PR rows otherwise lack a kind cue in the
explorer's mixed-kind list, where the `[forge_pr]` text chip is
the only signal today. The `⇄` glyph is the one PR cue that
survives once `kind_chip_span` collapses.

### H. Use `Y` as the default Fork glyph

Plain ASCII `Y` reads as "fork" universally and never has font
issues. Rejected as the default because it visually collides
with the letter `Y` in adjacent labels; `⑂` (OCR HOOK) is the
closest Unicode "fork" shape and renders 1-cell in the support
matrix. `Y` remains the documented ASCII override in
`[tui.theme.icons]` for terminals where `⑂` renders as `▯`.

## Open Questions Answered

- The glyph slate is Unicode geometric by default; Nerd Font
  users opt in through `[tui.theme.icons]`. No font detection.
- `AgentSession` keeps an independent kind glyph color
  (`node_agent_session`) layered with the existing harness pill;
  the two signals are not merged.
- `ForgePr` reuses the existing `pr_*` color fields rather than
  introducing a new `node_forge_pr` color.
- The icon override table is the one nested block under
  `[tui.theme]`; the color keys stay flat per ADR 0032.
- Glyph width is validated at parse time — overrides whose
  display width is not exactly 1 cell produce a warning and fall
  back to the default.
- JSON / DOT / HTML surfaces are out of scope for this ADR;
  H-VIS-005 will pick up the stable `NodeKind::display_label()`
  tag and any color propagation.
- `NO_COLOR` legibility is required and proven by a dedicated
  snapshot variant under H-VIS-006; the default slate is
  glyph-distinguishable without color.
