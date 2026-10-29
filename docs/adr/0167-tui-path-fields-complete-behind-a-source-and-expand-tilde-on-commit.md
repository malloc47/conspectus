# ADR 0167: Path Fields Complete Behind A Source, Ask For A Kind, And Expand `~` On Commit

## Status

Accepted. Carries out the `PathField` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying
its house rule 16 to every path field, and builds on the line editor and
text field of [ADR 0165](0165-tui-text-entry-one-line-editor-and-one-text-field.md).
Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `path_field` module and the test kit's `paths` module, and
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md)
with the standard `PATH_FIELD_KEYS` table. Narrows
[ADR 0097](0097-launch-spec-form-primitive.md): the launch spec's cwd is a
`PathField`.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, sections
2.2, 3, and 7; `inventory.md` section 4) found the tab-completed path widget
(`src/tui/widgets/path_omnibox.rs`) free of Conspectus imports but not
neutral in what it did:

- **No kind.** It suggested files as well as directories and validated
  with `exists`, so a file showed the green mark in a `cwd` field that
  `mux new`, `mux launch`, and the rebuild then refused as "not an
  existing directory".
- **I/O while drawing.** The validity mark ran a `stat` for every path row
  on every frame, and the hint ran a `read_dir` (and a `stat` per matching
  known path) on every frame while a path row was focused, with no way to
  substitute the file system or `$HOME`; its tests needed real
  directories and a real home.
- **A hint that disagreed with `Tab`.** Known paths matched by substring
  as well as prefix, and `Tab` replaced the whole value with the best one,
  while the hint showed a remainder only for a prefix match (or the tail
  of a basename), so typing `src` could show no hint and still jump to
  `/home/x/src/foo`.
- **Two hint rules.** The new mux and launch forms reserved 32 cells after
  every path value for the mark and an inline `Tab: <remainder>` hint,
  focused or not, which left a 60-column form a four-cell window onto the
  path; the pin edit form drew the mark alone and a fixed "Tab completes
  the path" line. The launch form's worktree repo, a directory, was a
  plain text field with neither mark nor completion.
- **Dead surface.** A `selected` index that was only ever 0, a
  `MAX_CANDIDATES` cap nothing consulted, and a `source` label and an
  `exists` flag on every suggestion that nothing drew.
- **Three `~` rules.** The widget, the legacy pin store
  (`legacy_pins.rs`), and the config loader (`config.rs`) each expanded a
  leading `~`, the same way. CSP-715 made the forms commit the widget's
  `expanded_value()` and left the three apart, the widget's to go to the
  framework.

## Decision

1. **`tuikit::path_field` owns the path field.** Its items: `PathField`,
   `PathSource`, `PathKind`, `Entry`, `Candidate`, `Completion`,
   `Validity`, `PathFieldView`, `expand_home`, and `collapse_home`, with
   `PATH_FIELD_KEYS` (`Tab` completes, over `PathKey`) kept with the
   standard tables in `keys` and re-exported. The host keeps the label,
   the known paths and their priorities, what `Enter` commits, whether a
   missing path may be committed, and the source over its file system.

2. **A read-only source.** `PathSource` answers three questions: a
   directory's entries (a name and a `PathKind` each), what is at a path
   (following links), and the home directory. The framework reads no file
   system and no environment of its own; the field holds the source as an
   `Arc<dyn PathSource + Send + Sync>`, so fields share one and the host's
   form states stay `Clone`. A relative path is passed on as typed. The
   test kit's `MemoryPaths` is an in-memory tree that counts the
   questions asked of it. Conspectus's `tui::paths::SystemPaths` reads
   `std::fs` (a link's kind from what it leads to; a broken link is left
   out) and `$HOME`.

3. **A kind.** A field asks for `PathKind::Dir` or `PathKind::File`. A
   directory field completes with directories only, and anything but a
   directory there is advisory. A file field completes with files and the
   directories that lead to them. Every Conspectus path field is a
   directory field.

4. **Completion only extends the typed text.** The candidates are the
   host's known `Candidate`s (higher priority first, then path order),
   then the source's entries of the typed path's directory whose names
   start with its last component (in name order), each path once, and
   only those that extend the trimmed text. `Tab` adds the first one's
   remainder and the hint shows that same remainder, so they cannot
   disagree. Nothing typed: every known candidate completes and nothing
   is listed. A completed directory gets no `/`. Typed text that starts
   with `~` completes in `~` form. `candidates()` is the ranked list.

5. **Looked at once.** The field asks its source when it is built, when
   its text changes, and when told to `refresh()`, and keeps the answers:
   the home, what is at the path, and the listing of the directory being
   typed in, which it keeps while only the last component changes.
   Validity, candidates, the expanded value, and drawing read only what
   it kept.

6. **An advisory mark.** `Validity` is `Unknown` (nothing typed, `?` in
   `inert`), `Exists` (there and of the field's kind, the success mark in
   `success`), or `Advisory` (missing or the other kind, the warning mark
   `▲` in `warning`). The two marks are `Severity::Success.mark()` and
   `Severity::Warning.mark()`, the shared `glyphs::marks::OK` and
   `glyphs::marks::WARNING`. The field takes any text.

7. **One `~` rule, committed.** `expand_home` expands `~` and `~/rest`
   against a home and leaves `~name` and everything else alone.
   `expanded_value()` (trimmed, expanded with the home the field kept) is
   what the new mux form, the launch form's cwd and worktree repo, and the
   pin edit form's path rows commit, and what the launch form reads its
   default command with. The legacy pin store and the config loader
   expand through the same function, so the three rules are one.

8. **One hint rule.** `PathFieldView` draws a field in exactly the width
   it is given: the text (a filled `TextField`), a space and the mark,
   and, only while the field is focused and a completion exists, a space
   and `Tab: <remainder>` in `accent`, the remainder cut in the middle.
   The hint takes at most 30 cells and at most half the row, the text has
   those cells back whenever no hint shows, and a hint that would get
   fewer than 8 cells is not shown. Every form draws its path fields
   through `form::path_cells` over this view.

9. **The host moves onto it.** `path_omnibox.rs`, its `OMNIBOX_KEYS` and
   `OmniboxKey`, `form::path_mark`, `completion_hint`, and
   `completion_remainder` are gone. The new mux cwd, the launch spec's
   cwd, the launch form's worktree repo (now completed from the same known
   directories), and the pin edit form's path rows are `PathField`s;
   `known_cwd_candidates` gives priorities without the unused source
   labels.

## Consequences

- **Visible changes** (the CSP-735 summary and the changelog list them):
  a file no longer passes for a directory (the warning mark, not `√`); an
  advisory path shows `▲` where it showed `!`; an unfocused path field,
  or a focused one with nothing to complete, gives its value the cells the
  32-cell suffix kept (a 60-column form shows the whole path instead of a
  four-cell window), with the mark at the row's end; the pin edit form
  shows the inline hint while a path row is focused and has a completion;
  the launch form's worktree repo gains the mark and `Tab` completion;
  a long remainder in the hint is cut in the middle; and typing a name
  that only appears inside a known path (`src` for `/home/x/src/foo`) no
  longer completes to it.
- Opening a form looks at each path once; typing looks at the new path
  and lists a directory only when the directory being typed in changes.
  Drawing a frame does no file system I/O.
- The CSP-715 `~` tests pass unchanged: the forms still commit
  `expanded_value()`, now the framework's.
- `Form<M>` (CSP-736) builds its Path field kind on `PathField` and
  `PathFieldView`; the forms' rows stay in Conspectus's `form` helpers
  until then.

## Alternatives Considered

- **Keep substring matches and replace the whole value on `Tab`.** It
  keeps the jump from a bare name to a known path, but the hint cannot
  show what such a `Tab` does, so the two disagree again. A ranked list
  the user picks from (the CSP-455 seam) is the place for matches that do
  not extend the text.
- **The source passed in on every call** (a `&dyn PathSource` to
  `handle`, `refresh`, and `new`). No stored trait object, but every form
  that holds a path field would need the source threaded through its key
  handling, and `set_value` from a host's builder would need it too. A
  shared `Arc` is one allocation for every field the program has.
- **A generic source parameter** (`PathField<S>`). No dynamic dispatch,
  but every form, and the framework's coming `Form<M>`, would carry `S`.
- **A fixed hint reserve whenever the field is focused**, or always (the
  old forms). Steadier as completions come and go, but it takes the value's
  cells when there is nothing to hint, which at 60 columns left the path a
  four-cell window. The story asked for the reserve only while a hint
  shows.
- **Append `/` to a completed directory.** Repeated `Tab` would descend,
  but the committed value would end in `/` unless the user removed it,
  and a pin's cwd compared with the graph's would differ by the slash.
- **Keep `!` for advisory.** No glyph change, but the mark would stay the
  one warning mark in the program that is not the warning severity's.
- **Keep the host's two other `~` copies** (CSP-715's interim). They are
  outside the TUI, but they already link the framework and the rule is a
  pure function of a value and a home; one copy cannot drift.
- **Hide entries whose names start with `.`** until a `.` is typed. Shell
  habit, but a change to what the old widget offered that the story did
  not ask for; it is a seam on the control page.

## Open Questions Answered

- *Directory-only for a cwd?* Yes: every Conspectus path field asks for a
  directory, and a file there is advisory.
- *Is the committed value home-expanded?* Yes, through `expanded_value()`.
- *Single best remainder or a visible list?* The best remainder, drawn as
  the hint; `candidates()` is the ranked list a later list UI draws.
- *Which hint rule?* Inline, only while focused with a completion, in a
  reserve of at most 30 cells and half the row.
- *What happens to `selected`, `MAX_CANDIDATES`, `source`, and `exists`?*
  Removed: the ranked list has no cursor until a list UI gives it one, no
  cap (the listing is kept sorted, so filtering it is cheap), no label
  (nothing drew it), and no per-suggestion `exists` (a listed entry
  exists; a known path is the host's word).
