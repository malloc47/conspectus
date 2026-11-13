# 0048: Path fields complete behind a read-only source, ask for a kind, and expand `~` on commit

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/path_field.md](../controls/path_field.md) |

## Decision

- **A read-only source.** `PathSource` answers three questions: a
  directory's entries (a name and a kind each), what is at a path
  (following links), and the home directory. The framework reads no file
  system and no environment of its own; a field holds its source as an
  `Arc<dyn PathSource + Send + Sync>`. The test kit's `MemoryPaths` is an
  in-memory tree that counts the questions asked of it.
- **A kind.** A field asks for a directory or a file. A directory field
  completes with directories only; a file field with files and the
  directories that lead to them.
- **Completion only extends the typed text.** The candidates are the
  host's known paths (higher priority first), then the source's entries of
  the typed directory that start with the last component, each path once.
  `Tab` adds the first one's remainder, and the hint shows that same
  remainder, so the two cannot disagree. A completed directory gets no `/`;
  text typed with `~` completes in `~` form.
- **Looked at once.** The field asks its source when it is built, when its
  text changes, and when told to `refresh()`, and keeps the answers;
  drawing reads only what it kept.
- **An advisory mark.** Nothing typed shows `?` in `inert`; a path that
  exists and is of the field's kind shows the success mark; a missing path
  or one of the other kind shows the warning mark. The field takes any
  text.
- **One `~` rule.** `expand_home` expands `~` and `~/rest` against a home
  and leaves `~name` and everything else alone; a field commits its
  trimmed, expanded value.
- **One hint rule.** `PathFieldView` draws the text, a space and the mark,
  and, only while the field has focus and a completion exists, `Tab:
  <remainder>` in `accent`, cut in the middle, at most 30 cells and half
  the row, and not shown with fewer than 8.

## Rationale

House rule 16. The host's path fields completed by substring and replaced
the whole value, so the hint and `Tab` disagreed; they read the file system
while drawing; and three copies of `~` expansion disagreed.

## Consequences

- A path field draws no I/O and can be tested against a memory tree.

## Alternatives considered

- **Substring matches that replace the value on `Tab`.** The hint cannot
  show what such a `Tab` does; a ranked list to pick from is the seam.
- **The source passed in on every call.** Every form would thread it through
  its key handling.
- **A generic source parameter.** Every form would carry it.
- **A fixed hint reserve.** It takes the value's cells when there is
  nothing to hint.
- **A `/` after a completed directory.** The committed value would end in
  one unless removed.
- **`!` for advisory.** The one warning mark that is not the warning
  severity's.
- **Hiding names that start with `.`.** A change nobody asked for; a seam.

## Origin

- host ADR 0167, "Path fields complete behind a source, ask for a kind, and
  expand `~` on commit": decisions 1 to 8.
