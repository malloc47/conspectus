---
id: CSP-424
title: 'Spike: evaluate `tui-pantry` as a widget-iteration harness'
status: Done
assignee: []
created_date: '2026-06-18 23:35'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 459000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: the TUI carries ~5.2k LOC of in-house widgets across
  `src/tui/widgets/` and visual judgement calls (column widths,
  chip placement, glyph spacing, narrow-pane truncation) currently
  iterate through `conspectus tui --snapshot` (ADR 0067) plus
  fixture diffs. That loop is excellent for regression coverage
  but slow for the "does this look right at 80 cols?" question
  the recent CSP-415..422 series kept asking. `tui-pantry` is a
  Storybook-style preview harness for ratatui widgets — boot one
  widget with chosen prop variants, no reducer, no fixture. The
  spike tests whether it shortens the visual-iteration loop
  enough to justify keeping.
- Scope (time-boxed, ~1 sprint):
    - Add `tui-pantry = "0.4"` to `[dev-dependencies]` and a
      `pantry.toml` config at the repo root.
    - Stand up a single binary target (`src/bin/pantry.rs` or
      `examples/pantry.rs` — decide during impl based on
      `cargo run --example` ergonomics vs `cargo run --bin`
      discoverability).
    - Port exactly one widget as the smoke test —
      `widgets/multi_select.rs` is the cleanest pure state
      machine and the lowest-risk port. Three prop variants:
      empty list, mid-selection, large list with scroll.
    - Spike outcome at the end: a one-paragraph note in the
      backlog entry recording (a) whether the visual loop felt
      materially faster than `--snapshot`, (b) how much glue per
      widget, (c) whether the pantry API's `pantry.toml` +
      ingredient model survives Conspectus's widget shapes
      without contortion, and (d) the go/no-go call.
    - If go: file follow-up stories per widget port (controls,
      pins-form, value modal, help legend, search, toast — six
      candidates) and a theme-harness ingredient that exercises
      every `[tui.theme]` key against the dark/light presets.
    - If no-go: rip out the dev-dep + binary + `pantry.toml`,
      record the lesson, close the story.
- Tests: none beyond the spike's own compile check —
  `cargo check --examples` (or `cargo check --bin pantry`) must
  pass and the existing `cargo nextest run --all-targets
  --all-features` must remain green with the new dev-dep present.
  The pantry binary itself is dev-time and is not gated by CI
  correctness tests.
- Risks / cons recorded up front:
    - `tui-pantry` is pre-1.0 (v0.4.0, ~53% docs coverage,
      single-org upstream taho-inc). Expect API churn; mitigated
      by the dev-dep posture — failure mode is `cargo update`
      breakage, not runtime regression.
    - Maintenance tax on the ingredient set: every ported widget
      is one more thing to keep in sync. Spike sizes that tax
      for one widget so the go-decision is informed.
    - Pantry's `Pane` primitive is a preview-cell frame, not an
      app-layout pane — does not address focus-chain or modal-
      routing pain. Those remain Tier 1 candidates from the
      prior ratatui-widget-library audit (`tui-textarea` on
      demand, `tui-popup` if framing duplicates).
- Open questions:
    - Binary vs example target: example is the canonical
      Cargo pattern for development harnesses; binary keeps the
      pantry close to the `src/` tree and reuses the workspace's
      clippy/fmt config without `--examples`. Decide during
      impl.
    - Whether the spike should run *before* a major widget audit
      (so the audit benefits from the loop) or *after* (so the
      audit informs which widgets are worth porting). Recommend
      before — the next H-UI-* and T8-* widget passes are the
      immediate beneficiaries.
- Blockers: none. Adopt during a quiet sprint before the next
  widget-heavy story (CSP-418 audit is the natural next
  customer if the spike lands go).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-20): **go**, scoped. Landed as `4f918aa`
(scaffolding) and recorded here. The smoke test
(`widgets/multi_select.rs` × 3 variants) compiles cleanly,
headless `--dump` renders each variant at arbitrary
dimensions, and the test suite stays green with the new
dev-dep. The four spike questions:
  - **(a) Visual loop materially faster than `--snapshot`?**
    Yes. `cargo run --example pantry -- --dump MultiSelect
    --variant "Mid-selection" --size 60x10` is a one-liner
    that gives the exact widget rendering with no UI state
    to compose, no key sequence to drive, and full control
    over the rect dimensions. Compare to
    `conspectus tui --snapshot --snapshot-keys
    'fjjjjjjjjjjj<Enter>' --snapshot-pane left` which
    requires choosing a fixture, knowing which key sequence
    reaches the desired sub-editor, and gets the
    widget-plus-app-chrome render. For
    "does this look right at 80 cols?" / "what does the
    empty state look like?" / "does this break at a 40-col
    modal?", the pantry loop is materially faster.
  - **(b) Glue per widget?** Modest. ~120 LOC for 3 variants
    of one widget — ~40 LOC per widget for the data struct
    + `Ingredient` impl + variant configs. Surfaces with
    more state variants (controls overlay, pin forms with
    error states) would land closer to 80–100 LOC each.
    Across the 7 currently-rich widgets (controls, pins
    forms, value modal, search, help, input, multi_select)
    the full porting tax is ~400–500 LOC of ingredient
    code — comparable to one of the Tier A swap commits.
  - **(c) `pantry.toml` + ingredient model fit?** Mostly
    yes; one workaround needed. The `Ingredient: Send`
    bound collides with our `MultiSelectState` which carries
    the upstream `ratatui_cheese`
    `Option<Box<dyn Fn>>` validator (`!Send`). Workaround:
    hold the configuration data on the ingredient, build
    state fresh inside `render()`. Cheap and works fine. The
    proc-macro `pantry_ingredients!` + `pantry.toml`
    `[ingredients]` convention is the canonical idiom but is
    not required — `tui_pantry::run!(ingredients_vec)` works
    as a single-file form, which is what the smoke test
    uses. Migration to the proc-macro form is a follow-up if
    the ingredient surface grows beyond ~5 widgets.
  - **(d) Go/no-go?** **Go.** Per-widget tax is manageable,
    theme bridging works (the multi_select widget renders
    through its existing `.theme()` builder; future
    ingredients can pass `Theme::default()` or a theme
    variant to preview operator palettes), headless dump
    gives deterministic per-variant rendering for free, and
    the dev-dep posture bounds the API-churn risk. Caveat
    on scope: not every widget benefits equally. Strong
    candidates for porting next — **controls** (sub-editor
    variants), **pin forms** (filled / empty / error
    states), **value modal** (small / large content),
    **search** (empty / no-matches / with-matches),
    **help** (the keymap legend already lives as data after
    CSP-429). Weak candidates: **badge**, **toast** (the
    latter retired its in-tree renderer under CSP-427).
    Filing a follow-up `CSP-437` to track per-widget
    ports as opportunistic work and a theme-harness
    ingredient that exercises every `[tui.theme]` key
    against the dark / light presets.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-044`
