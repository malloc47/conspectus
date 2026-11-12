# ADR 0208: The Claude Code Hook Install Edits settings.json In Place

## Status

Accepted. Settles the key-order question ADR 0206 left open (decision 9
and its Open Questions). Stays inside ADR 0143 category 5: the same file,
the same entries, written the same way (through a sibling temporary file,
through a symlink to its target, keeping the mode). The Codex install
already edits `config.toml` in place through `toml_edit` (ADR 0206
decision 17); this decision covers Claude Code's `settings.json`.

## Context

`hook init claude-code` and `hook remove claude-code` read
`settings.json` into a `serde_json::Value`, change the `hooks` entries,
and wrote the whole value back with `to_string_pretty`. The values
survived, but the text did not:

- Without its `preserve_order` feature, `serde_json` keeps an object's
  keys in a `BTreeMap`, so every object in the file, the operator's own
  hook groups and handlers included, came back with its keys sorted.
- The file was re-indented with two spaces, an escape such as `\u00e9`
  was written as the character, a number such as `1e1` as `10.0`, and a
  trailing newline was added where there was none.

Operators keep `~/.claude/settings.json` in dotfiles repositories, and a
project's `.claude/settings.json` is committed. The first install then
showed a diff of the whole file though only the hooks changed, and a
remove did not give the original back. The live-state install (ADR 0206)
writes 14 entries and is re-run to upgrade, so the cost recurs.

The project's rules ask for a decision before a new dependency or a
crate-wide convention, so the options were measured before choosing.

## Decision

1. **The settings text is edited in place.** The install and the removal
   still compute the new settings as a `serde_json::Value`, by the rules
   of ADR 0206 decisions 14 and 15. A splice (`src/cli/json_splice.rs`)
   then compares the value the file held with the new one and edits the
   text only where they differ:
   - an entry that is gone is cut out with the separator before it when
     it is its container's last item, else with the one after it;
   - a new entry is appended after its container's last item, laid out
     like the items already there: on a line of its own at their
     indentation, or on the same line in a one-line container. An empty
     container's first entries go one indentation level deeper than the
     line it opens on;
   - a changed value (a matcher, a command) is replaced where it stands;
   - array elements are matched by equality first, and each one left
     over is paired, in order, with the new element most like it, so a
     group of the operator's that loses a Conspectus handler is edited,
     not rewritten.

   Every other byte stays as the operator wrote it: key order,
   indentation (spaces or tabs), line endings, escapes, number spellings,
   and a trailing newline or its absence. New text takes the file's own
   indentation unit (two spaces when the file shows none) and line
   ending; the keys inside a new entry come in `serde_json`'s order, as
   before.
2. **An install followed by a remove gives the file back** byte for byte,
   since a cut takes exactly what an append added. That holds for a file
   with no hooks and for one with hooks of the operator's. An install
   that upgrades an older Conspectus entry in place (its matcher, a
   timeout) is not undone by a remove, which takes the whole entry out,
   as before.
3. **The result is checked before it is written.** The edited text is
   parsed back and must equal the intended value, or the command fails
   and writes nothing, as the Codex install does with its text. A dry
   run builds the same text, so it fails where the real run would. A
   file that does not exist yet is written pretty-printed, as before.
4. **Strict JSON only.** A file `serde_json` cannot parse (comments,
   trailing commas, a byte-order mark) is refused with the parse error,
   as before, rather than guessed at. An object being edited that holds
   a key twice is refused too, since no edit keeps both of the operator's
   values.
5. **No new dependency and no crate-wide feature.** `serde_json` stays
   without `preserve_order`. The splice is the crate's own code, about
   800 lines with its scanner and comments, and has its own tests, among
   them seeded randomized edits and round trips.

## Consequences

- After `hook init`, the operator's diff of `settings.json` shows only
  Conspectus's entries, and `hook remove` gives back the file they had.
- The project owns a small JSON scanner and diff. The parse-back check
  turns a defect in it into a refused write, never a damaged file, and
  its tests cover two- and four-space and tab indentation, CRLF, one-line
  files, a missing trailing newline, unicode, and the install, upgrade,
  and remove paths in both scopes.
- `serde_json::Map` keeps sorting keys everywhere else in the crate, so
  no machine-readable output changes.
- A settings file with comments is still refused, as it was.

## Alternatives Considered

- **A: enable `serde_json`'s `preserve_order`.** `indexmap` is already in
  `Cargo.lock` and already a declared dependency of the crate, so the
  feature adds no crate. But Cargo unifies features across the build, so
  it changes every `serde_json::Map` in the crate, not only the settings
  file's. Measured on 2026-11-12 by enabling it and running the full
  suite: 10 golden tests failed, three for `conspectus search --format
  json` and seven for the graph payload the HTML export embeds, whose
  keys came out in insertion order instead of sorted; the other 5,372
  passed. Every object built with `json!` or `to_value` (about 470 call
  sites under `src/`) changes order with it, and `Map::remove` becomes a
  swap-remove that reorders the keys left unless each call becomes
  `shift_remove`, the two in the Claude Code removal among them. Even
  then the file would be re-indented and its escapes and numbers
  rewritten. Rejected: a crate-wide behavior change and a change to
  machine-readable output, for a partial fix.
- **B, narrowed: replace only the `hooks` value.** Find the top-level
  `hooks` member and re-serialize its value alone. Every other key stays,
  but the operator's own hook groups inside it would still be re-sorted
  and re-indented. Decision 1 goes one level further at little extra
  cost.
- **A format-preserving JSON crate** (a parser with a concrete syntax
  tree, such as `jsonc-parser`'s). Would also keep comments, but adds a
  dependency, with its own ADR, for one file whose format is JSON.
- **C: change nothing and document it.** Keeps the code and tells the
  operator the keys come back sorted. Rejected: the noise recurs on every
  upgrade, and it is why the story exists.
- **A Claude Code plugin instead of settings edits** (ADR 0206,
  Alternatives). Still parked: it writes outside ADR 0143 category 5.

## Open Questions Answered

- *Should a settings file with comments be edited?* No. It is refused as
  before. Accepting comments would take a parser for the rules as well
  as the scanner, and a reason to believe Claude Code reads such a file.
- *Does the splice apply to Codex?* No. `config.toml` is already edited in
  place through `toml_edit`, which keeps comments and layout.
