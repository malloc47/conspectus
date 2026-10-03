---
id: CSP-433
title: 'Spike: evaluate `rat-widget` as a cohesive widget kit'
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
ordinal: 372000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `rat-widget` 3.2.1 (MIT/Apache, ratatui 0.30) is
  the widget half of `rat-salsa`, usable standalone as pure
  `StatefulWidget`s. It covers input / date / calendar / table /
  dialog / button / checkbox / radio / slider under shared
  focus / scroll / event traits (`rat-focus`, `rat-scrolled`,
  `rat-event`). The single upstream that could plausibly
  absorb multiple in-tree widgets (controls form, pins forms,
  future settings UI) under one design system.
- Spike outcome (2026-06-19): **no-go from structural
  analysis.** The risk-paragraph criterion ("verify the
  individual widgets are usable without the full
  rat-event / rat-focus / rat-scrolled trifecta — partial
  adoption is the only sustainable mode") fails empirically
  before the parallel implementation is even written. Findings:
    1. **Mandatory dep graph.** Adding the crate pulls in 14
       new transitive crates: rat-event, rat-focus,
       rat-scrolled, rat-popup, rat-menu, rat-ftable, rat-text,
       rat-cursor, rat-reloc, regex-cursor, regex, bstr,
       format_num_pattern, unicode-display-width,
       unicode-segmentation. Four rat-* deps bring substantial
       new code paths beyond what the kit itself ships.
    2. **State types carry the trifecta as mandatory fields.**
       `rat-text::TextInputState` has `pub focus: FocusFlag`
       baked into the struct (rat-text-3.1.0:112). The struct
       impls `HasFocus`, `HasScreenCursor`, `RelocatableState`,
       and `HandleEvent<Event, Regular | MouseOnly | ReadOnly,
       TextOutcome>`. You can construct with
       `FocusFlag::default()` and skip the focus chain, but
       the field is non-optional and the traits are foundational
       — you're working *against* the kit's design.
    3. **The Form widget brings its own layout vocabulary
       and state container.** From the kit's own example:
       `LayoutForm::new().spacing(1).flex(Flex::Legacy)
       .min_label(10)`, per-widget registration via
       `form_layout.widget(id, FormLabel::Str("..."),
       FormWidget::Width(22))`, render orchestration via
       `form.render(id, || TextInput::new(), &mut state.text1)`.
       A new layout language to learn vs. our manual line/span/
       Rect math.
    4. **Adoption is all-or-nothing per surface.** You can't
       mix our `widgets::input::TextInputState` (which wraps
       `tui-input`) with `rat_widget::form::Form`. The form
       widget orchestrates per-field render via upstream state
       ids, so the in-tree TextInputState would need to retire
       per-surface. For the pin create form alone, that's 8
       field-state conversions; across pin edit / rebind / bind
       / remove + rename overlay + controls MaxAge sub-editor
       + search overlay query, ~30 conversion sites total.
    5. **Event model mismatch.** Our reducer takes crossterm
       events and dispatches via `handle_key(KeyEvent) ->
       Outcome`. `rat-event` uses `HandleEvent<Event,
       Regular | MouseOnly | ReadOnly, Outcome>` — a
       parameterized trait + focus-aware outcomes. Bridging
       these means writing a translation layer per widget.
  Conclusion: the Tier C strategic bet does not pay off. The
  kit posture would force adoption of rat-salsa's design
  system (focus chains, event handlers, layout vocabulary) as
  the price of a unified widget design — exactly the
  "framework-tier" posture H-WIDG-* explicitly passed on at
  Tier D for `rat-salsa` proper. The dev-dep was added,
  dependency graph inspected, then ripped out cleanly
  (`Cargo.lock` reverted, working tree clean — no code
  landed).
- Followup: H-WIDG-* stays on the "narrow, drop-in" posture
  that has produced the Tier A wins (multi_select, toast,
  popup_frame, help binding). If a future surface (settings
  UI, calendar input for pin scheduling, table-backed pin
  catalog) genuinely needs a kit-shaped primitive, re-evaluate
  a single rat-widget primitive (not the form widget) under a
  fresh spike with the structural findings as the rubric.
- Blockers: `CSP-425`. [met]
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-009`
