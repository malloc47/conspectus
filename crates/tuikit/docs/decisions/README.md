# Framework decisions

The decisions that govern this crate: its boundary, its dependencies, its
control set and house rules, the design of each control, its conventions,
and its independence rules. Each record is short and stands alone: what was
decided, why, what follows from it, what else was weighed, and where the
decision came from. The architecture guide ([architecture.md](../architecture.md))
and the reference pages ([controls/](../controls/_template.md)) say how the
crate works today; this log says why it works that way.

## Canonical source

The framework grew inside a host application, and most of these decisions
were first made there, in the host's own decision records, beside decisions
about the host's domain and in its vocabulary. This log restates each
framework decision in framework terms and names its origin by number.

- **Before separation**, while the crate lives in its host's repository, the
  host's records are canonical for every decision that originated there. A
  record here restates the decision and points at its origin; when the two
  disagree, the host's record wins and the record here is corrected in the
  same change. A decision that changes what the host does as well gets a
  host record first and a record here in the same change. A decision about
  the framework alone (its documentation, its tests, its internal
  conventions) may be recorded here alone, and that record is canonical from
  the start.
- **After separation**, when the crate has a repository of its own, this log
  is canonical. The host numbers in each record's Origin section become
  history, and the host's records keep pointers to this log.

Record [0070](0070-the-decision-log.md) is this policy as a decision.

## Reading a record

A record is `NNNN-short-title.md`, numbered in this log's own sequence from
0001. It opens with `# NNNN: Title` and a header table with its `Status`
(`Proposed`, `Accepted`, or `Superseded by NNNN`) and, where one helps, the
`Page` that describes the code. Its sections, in order:

- **Decision**: what was decided, in a few points.
- **Rationale**: the problem and why this answer.
- **Consequences**: what follows for controls, hosts, and contributors.
- **Alternatives considered**: what was weighed and why it lost.
- **Origin**: one item per host record the decision came from,
  `- host ADR NNNN, "A neutral title": the decisions it contributed`, or a
  paragraph starting `None:` that says why there is none. The titles are
  written for this log: the host's own titles may use words the crate does
  not.

## Records

The Origin column lists the host record numbers each record restates; the
host's index marks each of them as framework-owned.

### The crate and its boundary

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0001](0001-a-crate-of-its-own.md) | The framework is a crate of its own, and its host depends on it one way | Accepted | 0150, 0152 |
| [0002](0002-components-and-conventions-not-the-runtime.md) | The framework holds components and conventions; the runtime stays with the host | Accepted | 0150, 0200 |
| [0003](0003-independence-is-tested.md) | Independence from the host is enforced by guard tests | Accepted | 0150, 0152 |
| [0004](0004-controls-return-intents-and-read-no-global-state.md) | A control performs no effect and reads no global state | Accepted | 0024, 0085, 0150 |
| [0005](0005-consistency-first.md) | A control that moves in unifies on the best variant and catalogs what users see change | Accepted | 0150, 0151 |
| [0006](0006-the-control-set.md) | The control set | Accepted | 0151 |
| [0007](0007-eighteen-house-rules.md) | Eighteen house rules, kept on one page | Accepted | 0151 |
| [0008](0008-the-third-party-crates.md) | The framework owns a small set of third-party crates | Accepted | 0024, 0030, 0152 |
| [0009](0009-rich-content-behind-a-feature.md) | Rich content sits behind an optional feature | Accepted | 0025, 0051, 0054, 0152, 0181 |
| [0010](0010-immediate-mode-and-no-component-framework.md) | Controls draw in immediate mode on ratatui, under no component framework | Accepted | 0024, 0085 |

### Foundations

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0011](0011-theme-tokens-and-named-styles.md) | The theme is typed role tokens plus styles the host registers by name | Accepted | 0032, 0153 |
| [0012](0012-two-muted-looks-and-alias-keys.md) | Two muted looks, and older keys load as aliases with one precedence | Accepted | 0153 |
| [0013](0013-one-style-grammar-and-soft-overrides.md) | Style values have one grammar, and overrides never stop a program | Accepted | 0022, 0032, 0153 |
| [0014](0014-no-raw-styles.md) | No raw styles outside the theme | Accepted | 0153, 0159, 0164 |
| [0015](0015-glyphs-are-measured-and-vetted.md) | Every glyph is one measured cell, in a table the host extends | Accepted | 0073, 0154 |
| [0016](0016-shared-marks-are-defined-once.md) | Shared marks are defined once, and a guard keeps them so | Accepted | 0154, 0168, 0179, 0187, 0190 |
| [0017](0017-text-is-measured-in-cells.md) | Text is measured, cut, wrapped, and painted in terminal cells by one module | Accepted | 0155, 0164, 0180 |
| [0018](0018-pieces-of-a-line-give-way-by-one-rule.md) | The pieces of a line give way by one rule | Accepted | 0188, 0190 |
| [0019](0019-one-scroll-window-and-one-follow-rule.md) | One scroll window, one follow rule, and windows returned by value | Accepted | 0085, 0156 |
| [0020](0020-one-scrollbar.md) | One scrollbar, ratatui's, in a gutter carved only on overflow | Accepted | 0076, 0156 |
| [0021](0021-a-scroll-region-counted-from-an-anchor.md) | A cursorless scroll region counts its offset from an anchor | Accepted | 0181 |
| [0022](0022-chips-share-one-width-rule.md) | Chips share one width rule | Accepted | 0157 |
| [0023](0023-ages-read-on-one-ladder.md) | Ages read on one ladder | Accepted | 0157 |
| [0024](0024-one-modal-frame-and-named-sizes.md) | Modals share one frame and five named sizes | Accepted | 0158, 0166 |
| [0025](0025-the-pane-frame-and-the-split.md) | A pane is framed with the focus cue, and two panes split by one rule | Accepted | 0076, 0189 |
| [0026](0026-the-app-frame.md) | A screen draws in one order through an app frame | Accepted | 0161, 0189, 0190 |
| [0027](0027-lists-show-their-cursor-one-way.md) | Lists show their cursor one way and step by their kind | Accepted | 0159 |

### Keys, overlays, and help

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0028](0028-keys-resolve-through-tables.md) | Keys resolve through tables, and footers and help are generated from them | Accepted | 0123, 0160 |
| [0029](0029-characters-bind-as-typed.md) | A character binds as typed, and every list moves with Ctrl-N and Ctrl-P | Accepted | 0160, 0197 |
| [0030](0030-one-footer-grammar.md) | One footer grammar, one key notation, and the keys that leave last | Accepted | 0160, 0172, 0193 |
| [0031](0031-availability-lives-on-the-row.md) | Availability lives on the row, and resolving a key ignores it | Accepted | 0160, 0192 |
| [0032](0032-a-scoped-keymap.md) | A program's own keys are one scoped keymap with stable ids and no shadowing | Accepted | 0192 |
| [0033](0033-overlays-on-a-host-owned-stack.md) | Overlays draw themselves on one modal stack the host owns | Accepted | 0085, 0161, 0191 |
| [0034](0034-steps-and-flows.md) | A follow-up question is a step, and a flow closes together | Accepted | 0172, 0176 |
| [0035](0035-generated-help-pages.md) | Help pages are generated from key tables and read in a pager | Accepted | 0162, 0178, 0192 |

### Feedback

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0036](0036-feedback-has-four-severities.md) | Feedback is one type with four severities over three channels | Accepted | 0163 |
| [0037](0037-toasts-are-in-tree.md) | Toasts are in-tree and take their time from the host | Accepted | 0152, 0163 |
| [0038](0038-a-bounded-log-in-memory.md) | The log is bounded, collapses repeats, and lives only in memory | Accepted | 0105, 0163 |
| [0039](0039-the-clipboard-is-osc-52-to-a-sink.md) | The clipboard is OSC 52, written to a sink the host supplies | Accepted | 0056, 0163 |
| [0040](0040-one-modal-footer.md) | Every modal footer is one footer, with one look and one fit | Accepted | 0178, 0193 |

### The test kit

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0041](0041-the-test-kit-is-a-feature-gated-module.md) | The test kit is a module behind a feature, and the crate tests itself with it | Accepted | 0164 |
| [0042](0042-key-scripts-and-a-frame-driver.md) | Tests press key scripts and read frames through one driver | Accepted | 0067, 0164, 0194 |
| [0043](0043-style-aware-goldens.md) | Goldens keep every color and are accepted through a writer the test passes | Accepted | 0164 |
| [0044](0044-the-conformance-suite.md) | A conformance suite holds every modal to the shared conventions | Accepted | 0164, 0176, 0191, 0193, 0194 |
| [0045](0045-source-scan-helpers.md) | The source-scan mechanisms are the test kit's, and the rule sets stay with their repository | Accepted | 0150, 0164 |

### Text entry

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0046](0046-one-line-editor-and-one-text-field.md) | One line editor over tui-input, and one text field that draws it | Accepted | 0030, 0165 |
| [0047](0047-one-text-prompt.md) | One text prompt, which explains its refusals | Accepted | 0166, 0176 |
| [0048](0048-path-fields.md) | Path fields complete behind a read-only source, ask for a kind, and expand `~` on commit | Accepted | 0167 |
| [0049](0049-forms.md) | Forms move on one ring, submit from any field, and keep their keys in view | Accepted | 0168 |
| [0050](0050-override-rows.md) | An override row follows an inherited value until it is edited | Accepted | 0170 |

### Choice and command

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0051](0051-action-menus.md) | Menus are one control whose entries are data | Accepted | 0171 |
| [0052](0052-dialogs.md) | Dialogs are one control in three forms | Accepted | 0151, 0172 |
| [0053](0053-checklists.md) | Checklists select all or none and count on their border | Accepted | 0152, 0173 |
| [0054](0054-palettes.md) | A palette is a query over results the host ranks | Accepted | 0174, 0175 |
| [0055](0055-settings-panels.md) | A settings panel applies each change as it is made | Accepted | 0176 |

### Reading

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0056](0056-one-pager.md) | Readers are one pager over content the host lays out | Accepted | 0177, 0178 |
| [0057](0057-log-lists.md) | A log list reads the host's log, with the selected entry's detail in an embedded pager | Accepted | 0105, 0179 |
| [0058](0058-captured-screen-wrap-modes.md) | A captured screen is laid out in one of three wrap modes | Accepted | 0106, 0181 |

### Data display and chrome

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0059](0059-the-tree.md) | The tree keeps its state by identity and draws lines its host lays out | Accepted | 0182, 0186, 0195 |
| [0060](0060-column-schemas.md) | Rows come from column schemas, and one policy decides what gives way | Accepted | 0183, 0184, 0185, 0186, 0195, 0196 |
| [0061](0061-the-property-grid.md) | Detail rows are a property grid, and a row reports what it asks for | Accepted | 0187 |
| [0062](0062-the-explorer.md) | The explorer is a cursor and a drill stack over a source the host supplies | Accepted | 0188 |
| [0063](0063-stacked-master-detail.md) | A stacked master-detail layout sizes its list to its rows | Accepted | 0188 |
| [0064](0064-tab-bar-and-priority-line.md) | The tab bar and the priority line fit by the one rule | Accepted | 0190 |
| [0065](0065-empty-states.md) | An empty slot says so one way | Accepted | 0190 |
| [0066](0066-the-status-bar.md) | The status bar names the keys of the scope that has them | Accepted | 0191 |

### Working on the crate

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0067](0067-documentation-is-mandatory.md) | The documentation is mandatory and held to the code | Accepted | 0150 |
| [0068](0068-pages-are-doctests.md) | Every page's Rust code compiles and runs as a doctest | Accepted | none |
| [0069](0069-code-hygiene.md) | Comments carry rationale, large test modules live beside their code, and drawn text cites no developer artifact | Accepted | 0100, 0124 |
| [0070](0070-the-decision-log.md) | The framework keeps its own decision log, canonical once it leaves | Accepted | 0150 |
| [0071](0071-the-separation-is-rehearsed.md) | The move to a repository of its own is rehearsed on every change | Accepted | 0199 |

## Host records not included

These host records were read and deliberately left out: they decide what
the host does with the framework, not how the framework works. Where a
framework decision came out of the same work, the record that carries it is
named.

| Host records | Why not here |
| --- | --- |
| 0031, 0034, 0045, 0055, 0061, 0062, 0063, 0064, 0065, 0071, 0107, 0114, 0139, 0144, 0198 | The host's views: which lists it shows, how it groups, sorts, and filters them, what each view remembers, and where a reveal lands. The tree, column schemas, palettes, and the settings panel draw them (records 0054, 0055, 0059, 0060). |
| 0072, 0120, 0145 | The host's state glyphs and state words, and what they mean. The framework vets the glyphs a host registers (record 0015). |
| 0023, 0033, 0074, 0075, 0112 | What the host's detail pane and preview say about one thing (0033 was superseded before the framework existed). The property grid, the explorer, and the scroll region draw it (records 0021, 0061, 0062). |
| 0078 | What each of the host's chrome lines carries. How a line gives way is records 0018 and 0064, and which keys the status bar names is record 0066. |
| 0019, 0052, 0053, 0122 | The host's document viewer: its model, its parsers, and its toggles. Its reader is the pager (record 0056). |
| 0097, 0169 | The host's launch form, and the form primitive it replaced; the form control was unchanged by them (record 0049). |
| 0020, 0021 | The host's command-line tables and their column allocator, which stays its own (record 0060). |
| 0068, 0069, 0070, 0113 | The host's fixture modes, its showcase scenario, and its screenshots. |
| 0115, 0116 | When the host fetches remote data, and how it refreshes. |
| 0111, 0117 | The host's rule for reading the working directory, and its manifest of programs, sockets, signals, and write paths. The framework reads no working directory and starts and writes nothing, so it needs neither (record 0004). |
| 0118, 0119 | The host's short name and its logo. |
| 0007, 0010, 0016, 0017, 0109, 0110 | How the host's repository is built, tracked, distributed, and placed, and its toolchain. A separated framework decides its own. |

## Adding a record

1. **What belongs here.** A decision that changes the crate's public API
   shape, a house rule, its dependency set, its layering, its independence
   rules, or the conventions its contributors follow. Decisions about what a
   host does with the framework belong to the host.
2. **One record per decision**, in the shape above, numbered next in this
   log's sequence. Add its row to the table of its group in the same change.
   A record that supersedes another says so in both statuses.
3. **No host vocabulary.** The independence guard scans this directory like
   the rest of the crate, so a record that needs a host term says the
   generic thing instead and names its origin by number for the detail.
4. **A new third-party dependency needs a record first.** The record states
   the need, the alternatives compared, and the conclusion. The dependency
   allowlist in the independence guard changes in the same change and cites
   the record.
5. **Origins are checked.** While the crate lives in its host's repository,
   a record that names a host record needs the host's index to mark that
   record framework-owned (wholly or in part), and a host record of its
   interface section that no record restates belongs in the table above.
   `tests/doc_sync.rs` holds the index to the records and the records to the
   host's; when the crate stands alone it skips the host checks with a note
   ([contributing.md](../contributing.md#the-pages-and-the-code-stay-in-step)).
