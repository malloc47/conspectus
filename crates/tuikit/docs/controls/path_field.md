# `PathField`

| | |
| --- | --- |
| Module | `tuikit::path_field` |
| Layer | 1 (control) |
| Type parameters | none; the host's file system enters as a `PathSource` trait object, shared behind an `Arc` |
| Cargo feature | none (the test kit's in-memory source, `testkit::paths::MemoryPaths`, is behind `testkit`) |
| Outcomes | `PathField::handle` returns an `Edit`: `Tab` that completes is `changed` and `consumed`, `Tab` with nothing to complete is `Edit::IGNORED` (the host moves on), and every other key is the line editor's `Edit`; `PathFieldView` only draws |

## Purpose

`PathField` is a text field for a path: a `LineEdit` that completes the
path it holds, says whether it names something of the kind it asks for,
and expands a leading `~`. Every path a program asks for (a working
directory, a project root, a file to open) uses it, so each completes,
checks, and commits a path the same way.

It replaces what a path entry grows into one form at a time: a field that
suggests files where it wants a directory, and marks a file as fine there;
a mark that reads `~/src` as the home's `src` while the form commits
`~/src` as typed to a command that does not expand it; a hint that names
one completion while `Tab` jumps to another; a directory listing and a
`stat` on every frame drawn; and a reserve of hint cells that some forms
keep and others do not.

- **A source.** The field reads the file system only through the
  `PathSource` its host supplies: a directory's entries (`Entry`, a name
  and a `PathKind`), the kind of what is at a path, and the home
  directory. The framework reads no file system and no environment of its
  own; a source never writes. A test supplies an in-memory tree
  (`testkit::paths::MemoryPaths`), so it needs no real directory or home.
- **A kind.** A field asks for `PathKind::Dir` or `PathKind::File`. A
  directory field completes with directories only, and a file there is
  advisory, not valid. A file field completes with files and the
  directories that lead to them.
- **Ranked completion.** The candidates are every path that extends the
  typed text: the host's known `Candidate`s first (higher `priority`
  first, then path order), then the source's entries of the typed path's
  directory whose names start with its last component, in name order,
  each path once. `Tab` takes the first and adds its remainder, the cursor
  at the end; the hint shows that same remainder. Nothing typed: every
  known candidate completes, and nothing is listed. A completed directory
  gets no `/`; typing `/` lists its entries.
- **`~`.** `expand_home` is the one rule: `~` and `~/rest` are the
  source's home directory; `~name` and everything else stay as typed. The
  field checks and completes with it, typed text that starts with `~`
  completes in `~` form, and `expanded_value()` (trimmed, `~` expanded) is
  what a host commits. `collapse_home` writes a path under the home back
  as `~`.
- **A validity mark.** `Validity::Unknown` (nothing typed),
  `Validity::Exists` (there, and of the field's kind), or
  `Validity::Advisory` (not there, or the other kind). Advisory: the
  field takes any text; whether a missing path may be committed is the
  host's. `found()` says what was there, for a host that explains why.
- **Looked at once.** The field asks its source when it is built, when
  its text changes, and when told to `refresh()`, and keeps the answers:
  what is at the path, the home directory, and the listing of the
  directory being typed in, which is kept while only the last component
  changes. Reading the validity, the candidates, or the expanded value,
  and drawing, read only what it kept: drawing does no I/O.

The host keeps what the path is for: the label, which known paths to offer
and at what priority, what `Enter` commits and whether a missing path may
be committed, the source over its file system, and when the file system
has changed enough to refresh.

## Usage example

A form row for a working directory: the host's source over its file
system, the known directories first, `Tab` completing, and the committed
value expanded.

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuikit::path_field::{Candidate, Entry, PathField, PathKind, PathSource, Validity};
use tuikit::theme::Theme;

/// The host's file system: read-only, behind the trait.
struct Disk;

impl PathSource for Disk {
    fn entries(&self, dir: &Path) -> Vec<Entry> {
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        read.flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let kind = match std::fs::metadata(entry.path()) {
                    Ok(meta) if meta.is_dir() => PathKind::Dir,
                    Ok(_) => PathKind::File,
                    Err(_) => return None,
                };
                Some(Entry::new(name, kind))
            })
            .collect()
    }
    fn kind(&self, path: &Path) -> Option<PathKind> {
        let meta = std::fs::metadata(path).ok()?;
        Some(if meta.is_dir() { PathKind::Dir } else { PathKind::File })
    }
    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

let source: Arc<dyn PathSource + Send + Sync> = Arc::new(Disk);
let mut cwd = PathField::new("~/", PathKind::Dir, source.clone())
    .with_candidates(vec![
        Candidate::new("~/work/current", 100),
        Candidate::new("~/work/older", 50),
    ]);

// The form resolves its own `Tab` row first and asks the field to
// complete; nothing to complete means the next field.
let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
if !cwd.handle(tab).consumed {
    // move to the next field
}
assert_eq!(cwd.value(), "~/work/current");

// Drawing reads what the field kept: the text, the mark, and (focused,
// with a completion) the hint. A form passes its field look.
let theme = Theme::default();
let row = cwd
    .view()
    .focused(true)
    .marks(true)
    .placeholder("-")
    .style(theme.field_active.into_style())
    .line(60, &theme);

// Enter: the host commits the expanded path, and decides what an
// advisory one means.
let commit = cwd.expanded_value();
let exists = cwd.validity() == Validity::Exists;
```

The framework's own source of truth for these rules is its tests, which
run against `testkit::paths::MemoryPaths`: a tree built in a line
(`MemoryPaths::new().with_home("/home/me").dir("/home/me/src")`) that
counts every question asked, so a test can check that drawing asks
nothing (`paths.calls()` is unchanged).

### Layout

`PathFieldView` draws a field in exactly the width it is given:

1. **The text**, a filled `TextField` with the knobs the host sets on the
   view (`focused`, `marks`, `placeholder`, `style`), so the mark after it
   stays in one column.
2. **The mark**: a space and `Validity::mark()` in `Validity::style()`.
3. **The hint**, only while the field is focused and a completion
   exists: a space and `Tab: <remainder>` in `accent`, the remainder cut
   in the middle (it is a path). It takes at most 30 cells and at most
   half the row, and the text has those cells back whenever the hint is
   not shown. A row that would give the hint fewer than 8 cells shows
   none, and a row of two cells or fewer is the text alone.

This is the one hint rule: every form with a path field shows the same
hint in the same place, and no form reserves cells for a hint it is not
showing.

## Key table

`PATH_FIELD_KEYS`, over `PathKey` (a standard table; see
[keys.md](keys.md)), consulted before the line editor's
`LINE_EDIT_KEYS` (see [text_edit.md](text_edit.md)). It is a fragment of
a field, not an overlay's whole table, so it does not start with
`Ctrl-C`.

| Key | Action | Notes |
| --- | --- | --- |
| `Tab` | Complete | adds the best completion's remainder; with nothing to complete the key is not consumed, so a form moves to the next field; an alias, since the form's footer names `Tab` (`Tab complete/next` in `FORM_KEYS`) |
| any other key | (line editor) | edits the path; a key that changes it looks at the new path |

The hint's key is written from this table's row (`key_label`), so it
cannot name a key the field does not bind.

## Theme tokens

| Token | Used for |
| --- | --- |
| `inert` | the `?` mark of a field with nothing typed |
| `success` | the mark of a path that is there and of the field's kind (`Severity::Success`) |
| `warning` | the mark of an advisory path (`Severity::Warning`) |
| `accent` | the completion hint |
| `field_cursor` | the caret, through `TextField` |

The text, its fill, and the ground of its scroll marks take the style the
host passes (`PathFieldView::style`); a form passes `field_active` for the
focused field and `field` for the others. The marks are the shared
`glyphs::marks::OK` and `glyphs::marks::WARNING` (the marks of the
success and warning severities) and the ASCII `?`.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 8 | Footers and hints | the completion key is a key table; the hint writes the key from it, and the form's footer, generated from its own table, names `Tab` |
| 13 | Theme | the marks and the hint take theme tokens (through `Severity` for the two marks); no raw color or reversed cell |
| 14 | Width | the row, the reserve, and the hint are measured in cells; the remainder is cut in the middle with `…`, as a path is |
| 15 | Text editing | the path is a `LineEdit`, with the shared editing keys and paste rule |
| 16 | Path fields | a `PathKind` (a directory field takes no file), `~` expanded on commit through `expanded_value()`, and completion behind a `PathSource` whose answers are kept, so drawing does no I/O |

Not applicable: 1, 2, 10, and 12 (no list is drawn; the candidate list is
a seam), 3 and 5 (the field confirms and cancels nothing), 4 (the field
binds no `Esc`, `Ctrl-C`, or `q`; leaving is the form's), 6 (the form's
ring is `Form`'s), 7 (the mark is advisory, not a validation error; a
form that refuses a path puts its reason on its error line), 9 (the focus
cue is the pane frame's), 11 (the field gives no feedback), 17 (labels
are the host's copy), and 18 (pagers).

## Seams left open

- **A ranked candidate list.** `candidates()` is every completion, best
  first, with its `value()` and `remainder()`; the field draws only the
  first as its hint. A host that shows several (inline under the field,
  or in a sidecar beside the form) draws that list and lets the arrows or
  a repeated `Tab` choose; choosing is `set_value(completion.value())`.
  Showing where each came from (a known path or a listed entry) would add
  an origin to `Completion`.
- **A browse mode.** A directory picker (a tree of the source's
  directories, opened from the field) would read the same `PathSource`
  and hand its choice back with `set_value`; the field needs nothing new.
- **Relative paths.** A relative path goes to the source as typed, so
  what it is relative to is the source's. A field with a base directory
  of its own would join it before asking.
- **Refreshing on a schedule.** The field looks again only when its text
  changes or `refresh()` is called. A host whose files change under an
  open field (a directory a command just created) calls `refresh()`, on
  that event or on a timer.
- **Hidden entries and other filters.** Every entry of the kind asked for
  is listed, hidden ones included. A filter (hide dot entries until a `.`
  is typed, only directories holding a marker file) would be one more
  option applied where the listing is kept.
- **A live refusal.** The mark is advisory. A host that refuses a path on
  commit says why on its form's error line; one that refuses while typing
  would read `validity()` after each change.
