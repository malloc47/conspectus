# `ControlName`

> Template for a public module's reference page. Copy this file to
> `docs/controls/<module>.md`, replace every placeholder, delete the quoted
> hints, and add the page to `src/doc_pages.rs`. Keep every heading and every
> row of the table below, in this order, even when the answer is "none": the
> doc-sync test (`tests/doc_sync.rs`) and readers rely on one shape. Detail
> of your own (an API table, layout rules) goes in `###` sections under the
> usage example, or in `##` sections after the seams. Write without the
> host's domain vocabulary; the independence guard scans this directory (see
> [../contributing.md](../contributing.md#independence-guards)).

| | |
| --- | --- |
| Module | `tuikit::<module>` |
| Layer | 1 (component) or 2 (convention) |
| Type parameters | The parameters the control takes and what each stands for: `M` (host message), `Cx` (host context), `Id`, `R`; or "none" |
| Cargo feature | The feature that gates the control, or "none" |
| Outcomes | What handling a key can report, for example commit, close, consumed, ignored |

## Purpose

> Two or three sentences: what the control is for and which scattered
> variants it replaces. Say what stays with the host (labels, row content,
> what a commit does) so a reader knows where the adapter begins.

## Usage example

> A complete, minimal example that builds the control, feeds it a key,
> handles its outcome, and renders it. Define small local types for `M` and
> `Cx` so the example stands on its own, and show the theme being passed in.
> Every `rust` block on the page compiles and runs as a doctest, so assert
> what the example shows; a block that is not Rust takes another tag
> (`text`, `toml`).

```rust
// Build the control, pass it a key and the host context, and act on the
// outcome. Replace this sketch with a real example.
```

## Key table

> The control's keys as the key table defines them. The footer and the help
> page are generated from this table, so the rows here and the hints a user
> sees are the same data. Mark the keys the control shares with the standard
> tables.

| Key | Action | Notes |
| --- | --- | --- |
| `Esc` | Back | Steps back one level |

## Theme tokens

> Every token the control reads, and for what. List named-style registry
> lookups separately; the host registers those. A control with no raw colors
> reads only the tokens listed here.

| Token | Used for |
| --- | --- |
| `selection_active` | The highlighted row when the control is focused |

## House rules followed

> One row per house rule that applies, with how the control follows it. List
> the rules that do not apply with a reason, so a reader can tell "does not
> apply" from "forgotten". The rules are in
> [../house-rules.md](../house-rules.md).

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | Full-row highlight and pointer marker |

Not applicable: rule numbers, each with the reason.

## Seams left open

> Extension points the control keeps ready for needs that are not built yet,
> and what would plug into each. Examples: key rows that can carry
> availability, a rendered-area report for mouse hit testing, a second content
> source. If there are none, say "none" and why.
