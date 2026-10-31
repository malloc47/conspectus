# Gap Analysis: Controls Common Elsewhere, Not Yet In Conspectus

Phase 4 of the [TUI framework plan](README.md). It compares the controls and
conventions other TUI frameworks and well-regarded TUI apps ship against the
[proposed control set](control-set.md), to document what may be worth adding
later and what to skip. **Nothing here is scheduled work**; it seeds the
framework's roadmap document and tells the extraction stories which seams to
leave open.

Evidence: three surveys under [`research/`](research/), covering 60+ Rust
crates ([01](research/01-rust-ecosystem.md)), about 40 projects in Go,
JavaScript/TypeScript, C/C++, Haskell, Zig, .NET and others
([02](research/02-other-languages.md)), and Textual, Rich, prompt_toolkit,
urwid, web design-system component lists, and 15 TUI apps
([03](research/03-python-and-design-systems.md)).

**Limits.** The research agents' web search was rejected by the backend, so
they worked from direct page fetches (crates.io, docs.rs, pkg.go.dev, project
READMEs) dated 2026-10-27, and each file lists what it could not verify.
"Frameworks" counts are out of 6 Python/Go frameworks (file 03); "apps"
counts are out of 15 apps and are floors, because a control is counted only
when a fetched page mentions it. Where the Conspectus column says *none*, it
was checked against source (for example, there is no mouse, bracketed paste,
synchronized-update, or kitty-keyboard handling, and no user key remapping).

## 1. How To Read The Matrix

Conspectus column: **have** (in the proposed set), **partial** (some of it
exists), **none**. Verdict: **Tier 1** = cheap now or needed by an open
story; **Tier 2** = add when a feature needs it, keep the seam open;
**Tier 3** = feature-driven only; **Skip** = a poor fit for a terminal or for
this product.

## 2. Control Matrix

### Input

| Control | Commonness | Conspectus | Verdict | Reference to study or use |
| --- | --- | --- | --- | --- |
| Single-line text | ubiquitous (6/6; apps 8+) | have: `LineEdit`/`TextField` | done | `tui-input` stays the engine |
| Multi-line text area | common (4/6) | none | Tier 2 | `ratatui-textarea` (ratatui org, 0.9.x); `tui-textarea` 0.7.0 declares `ratatui ^0.29` and will not resolve against 0.30, so CSP-430 should retarget |
| Validated / typed input (number, regex, mask) | common (4/6) | partial: ad-hoc `parse_max_age` | Tier 2 | validators as `TextPrompt` `on_confirm` results (house rule 7); `rat-text` masks for vocabulary only |
| Autocomplete | common (3/6; apps: Posting, Helix, fzf, Atuin) | partial: path completion only | Tier 2 | generalize `PathField`'s `PathSource` into a `CompletionSource` when a second field kind needs it |
| Checkbox | ubiquitous (5/6) | have: `Checklist`, form toggles | done | house rule: distinct glyphs for checkbox and radio (today both render `[x]`) |
| Radio / one-of-N | common (4/6) | have: `OneOf` | done | same glyph fix |
| Switch / toggle | rare (1/6) | none | Skip | a labelled checkbox serves |
| Dropdown select | common (3/6) | partial: harness chips, `ChoiceField` | Tier 3 | only if a field has too many options for chips |
| Multi-select with select-all | occasional | have: `Checklist` (no select-all) | Tier 1 | add select-all/none and a selected count when `Checklist` lands |
| Fuzzy picker with preview | common (apps 6/15) | have: `Palette`, quick select | done | preview pane for picker rows is a possible `Palette` option |
| File picker / browser | common (3/6) | partial: `PathField` | Tier 3 | CSP-434 (`ratatui-explorer`); browse mode as a `PathField` companion |
| Date/time picker, slider, stepper, color picker | rare or absent in TUIs | none | Skip | web-only conventions |
| Form container with validation summary | rare (1/6; Huh) | have: `Form<M>` | done | Huh's group/skip/validate-before-advance model for any later wizard |

### Choice and command

| Control | Commonness | Conspectus | Verdict | Reference |
| --- | --- | --- | --- | --- |
| Command palette (named actions, fuzzy, scoped) | occasional (2/6; apps Helix, Posting) | none (CSP-390/391/392 open) | **Tier 1** | becomes a `Palette<R>` instance fed by the key tables' action ids; Textual's providers with `search`/`discover` show the scoring shape |
| Which-key / pending-chord popup | occasional (apps Helix, Yazi) | none | Tier 3 | only if multi-key chords are introduced |
| Action menu / popup menu | common (4/6; apps 5) | have: `ActionMenu` | done | |
| Context menu at the pointer | not seen | n/a | Skip | |
| Tri-state action availability (show / hide / dim) | Textual only | none | **Tier 1** | a key-table row gains `available`, so footers, help, and menus dim unavailable actions from one source (Textual `check_action`) |

### Display and data

| Control | Commonness | Conspectus | Verdict | Reference |
| --- | --- | --- | --- | --- |
| List | ubiquitous | have | done | |
| Tree | common (4/6; apps 5) | have: `TreeView` | done | Bubbles v2 now ships a `tree`; `tui-tree-widget` is the Rust standard but fits the explorer poorly (CSP-435) |
| Table / data grid with sort and filter | common (4/6) | partial: CLI `ColumnSpec` tables; TUI trees only | Tier 2 | share one column schema between the CLI renderer and `TreeView` (inventory 02 §6); among the Go libraries surveyed none has a complete sortable, filterable table, and Textual's `DataTable` is the broadest one found |
| Pager / viewport with find | ubiquitous | have: `Pager` | done | |
| Log viewer | occasional | have: `LogList`, `Pager` | done | `tui-logger` is tied to `log`/`tracing` capture, not needed |
| Markdown / rich text | common (3/6) | have: `richtext` | done | `tui-markdown` calls itself a proof of concept; keep behind a feature |
| Syntax-highlighted code view | common (3/6; apps 4) | have via `syntect` | done | |
| Diff viewer | rare (OpenTUI only) | none | Tier 3 | only if the product shows worktree or PR diffs |
| JSON / pretty-print viewer | common (3/6) | none | Tier 3 | the `Pager` plus a content adapter covers a read-only view |
| Spinner | common (3/6) | partial: braille spinner swap in rows | **Tier 1** | CSP-432 (`throbber-widgets-tui`); a framework `Spinner` with the glyphs vetted in `glyphs` |
| Progress bar | common (4/6) | partial: `Progress{scanned,total}` status text in content search | Tier 2 | a `Progress` control when a second long operation appears |
| Sparkline / inline graph | occasional (2/6; apps btop, bottom, lazydocker) | none | Tier 3 | ratatui ships `Sparkline`; only if activity-over-time becomes a column |
| Breadcrumb | rare (1/6: Terma) | have: explorer breadcrumb | done | no standalone Rust crate; it appears only inside large kits |
| Collapsible / accordion | rare (1/6) | have: disclosure rows | done | |
| Tabs | occasional (2/6) | have: `TabBar` (a view strip) | done | |
| Resizable split pane | rare | partial: fixed 50/50 split, stacks when narrow | Tier 2 | CSP-319 responsive layout; FTXUI and `ratatui-interact` for the drag model |
| Hyperlink (OSC 8) | occasional | none | Tier 3 | never make a target reachable only through the link |
| Image / big text / charts | occasional | none | Skip | not in this product's domain |
| Empty state / skeleton | rare | have: `EmptyState` | done | gh CLI's empty-state rule |
| Background task list | rare (Yazi) | partial: in-flight chip, Messages log | Tier 3 | |

### Overlays and feedback

| Control | Commonness | Conspectus | Verdict | Reference |
| --- | --- | --- | --- | --- |
| Modal dialog | common (4/6) | have: `Dialog` | done | typed result on dismissal (Textual `dismiss(result)`) is what `Outcome<M>` already is |
| Toast | occasional (2/6: Textual, Terma) | have: `Toasts` + severity | done | the Go, JS, and other-language set (file 02) had no in-app toast component, only desktop-notification escapes and static status messages |
| Tooltip | occasional (2/6) | none | Skip | a terminal has no hover; key-hint footers do this job |
| Help overlay (`?`) | common (apps 7/15) | have: `HelpPage` | done | |
| Key-hint footer | common (3/6) | have: `StatusBar` | done | grouped hints (Textual `Binding.group`) are a refinement |

### Cross-cutting capabilities

| Capability | Commonness | Conspectus | Verdict | Reference |
| --- | --- | --- | --- | --- |
| Theme / palette presets | ubiquitous (6/6; apps 10) | partial: one default, user overrides, no presets (CSP-269) | **Tier 1** | semantic role sets converge on fg/bg, muted, accent, border (+ focused), selection, success/warning/error/info; Terminal.Gui derives roles from a mandatory `Normal` |
| Light/dark auto-adaptation | common (apps 5/15) | none | Tier 2 | mode 2031 notifications; Huh passes `isDark` into the style bundle |
| `NO_COLOR` and color-depth downgrade | convention | partial: CLI `--color` honors `NO_COLOR`; no truecolor/256/16 detection | **Tier 1** | `tui-nuances` and `ratatui-themekit` do both; house rule 1's pointer marker is the non-colour carrier |
| Non-colour status carriers | guidance | have: glyph plus state word | done | never colour as the only meaning |
| Accessible / non-interactive mode | rare (Huh, Ink) | none | Tier 3 | the CLI already offers `--format` output; a static-text TUI fallback is low value for a survey tool |
| Mouse support | ubiquitous (apps 9/15) | none | Tier 2 | hit testing is the universal convention, with every crate inventing its own type; leave a `rendered_at(Rect)` seam on each control and add a click-region registry when the first mouse story lands (Lip Gloss v2 compositor, `ratatui-interact`) |
| Custom key bindings (user remap) | common (3/6; apps 9) | none | Tier 2 | key rows with stable ids allow overrides; collision detection at construction (Brick, Textual `id`) |
| Bracketed paste | convention | none | **Tier 1** | house rule 15; crossterm supports it |
| Synchronized updates, kitty keyboard protocol | convention | none | Tier 3 | crossterm exposes them; the kitty protocol is what lets `Ctrl-I` and `Tab` differ |
| Clipboard (OSC 52) | occasional | have | done | `Write` sink (layer 2) |
| Undo | rare | have (domain) | stays | |
| Density (compact flag) | design-system idea | none | Tier 2 | CSP-191/270 density modes; a `density` token on `theme` |
| Animation / effects | occasional | none | Skip for now | `tachyonfx` composes as a post-render pass over any widget buffer, so deferring costs nothing |
| Snapshot / pilot testing with colour | occasional | partial: `tui --snapshot` keeps ANSI; insta goldens drop styles | **Tier 1** | `TestBackend` plus insta cannot assert colour (ratatui #1402); the PTY harnesses `termlens` and `termwright` and `tui-pantry dump` are the Rust alternatives; teatest and OpenTUI's test renderer show the layering |
| Plugins / extension model | common (apps 6/15) | none | Skip | out of product scope |

## 3. Ranked Gaps For The Roadmap

**Tier 1** (cheap, or an open story already needs it): command palette as a
`Palette` instance; tri-state action availability in the key tables; a
`Spinner` control; theme presets plus `NO_COLOR` and color-depth downgrade;
bracketed paste; select-all/none in `Checklist`; style-aware snapshot goldens.

**Tier 2** (keep the seam open): multi-line text area via `ratatui-textarea`;
validated input; a general completion source; a shared column schema with the
CLI tables; a progress bar; a resizable split; light/dark adaptation; mouse
hit testing; user key remapping; a density token.

**Tier 3** (feature-driven): which-key popup, dropdown select, file browser,
diff and JSON viewers, sparklines, hyperlinks, background task list,
synchronized updates, kitty keyboard.

**Skip:** switch, slider, date picker, stepper, color picker, pagination,
tooltip, context menu, image and chart widgets, plugins, animation (for now).

## 4. Where Conspectus Is Already Ahead Or Level

- **Key tables that also generate help and footers** (CSP-468). Brick,
  Textual, Terminal.Gui, and Bubbles `key`+`help` are the best designs found;
  published Rust equivalents are young (`tuika`, `schemaui`,
  `tui-file-explorer`). The framework's `keys` module is on the leading edge,
  and the useful additions are Brick's collision detection and Terminal.Gui's
  single `Shortcut` object feeding status bar, menu bar, and popup menu.
- **Intent outcomes instead of effects.** Widgets that return an outcome and
  let the host act (`Selected`/`Dismissed`, `Submitted`/`Cancelled`) are the
  convention in the better Rust kits; the `Overlay` contract already works
  this way.
- **A cross-control consistency suite.** No surveyed kit ships anything like
  it; `ratcn`'s `copy-fixture` independence check is the closest thing to the
  independence guard planned here.
- **ANSI-preserving snapshots for agents** (`tui --snapshot`) are in the same
  family as `tui-pantry dump` and the PTY harnesses.

## 5. Dependency Hygiene Notes From The Survey

- Prefer ideas over dependencies for the young design kits (`tuika`,
  `tui-lipan`, `ratatui-themekit`): 0.x, first released in 2026, high churn
  (one released 95 versions in five months), small download counts.
- `FrankenTUI` (`ftui`) requires a pinned nightly toolchain and its license
  badge reads "MIT with an OpenAI/Anthropic Rider"; read it for ideas only.
- Stale or abandoned: `tui-textarea` 0.7.0 (two ratatui minors behind),
  `cursive` (last release 2024-08), `dioxus-tui` (2024-02), `rat-widget` as an
  all-or-nothing choice (CSP-433 stands).
- `ratatui-comfy-toaster` is young; the plan replaces it with in-tree code.

## 6. A Finding Outside The Framework's Scope

Bubble Tea v2.1.0 (2026-10-08) added a terminal "Program Status Protocol"
(OSC 7501) through which a program reports `idle`, `working`, `blocked`,
`done`, or `error`; OpenTUI and libvaxis implement it too, and one release
summary describes it as aimed at AI agents. That overlaps Conspectus's
agent-state domain: it could eventually be an evidence source for session
state. It belongs to the product roadmap, not the framework, and the survey
read it from release pages only, so verify it against the protocol
specification before relying on it.

## 7. How The Plan Uses This

- The `Palette`, `keys`, `Checklist`, `Toasts`, `theme`, and `testkit` stories
  each carry a *seam left open* acceptance item for the Tier 1 and Tier 2
  entries that touch them, so adding the control later does not break the
  API.
- The framework's roadmap document, `crates/tuikit/docs/roadmap.md`
  (CSP-761), is this matrix, trimmed to the framework's own terms, with
  each Tier 1 item marked done or open.
- Tier 3 and Skip rows get no stories.
