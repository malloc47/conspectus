# ADR 0021: `conspectus table <ROWS>` Command And Config Schema

## Status

Accepted.

## Context

H-TBL-001 through H-TBL-005 modernized the renderer behind
`conspectus session`. The renderer now serves more than agent sessions —
PR identifiers, fork lineage, mux session names, and checkout cwds all
appear as cells. Once H-TBL-008 / H-TBL-009 add `prs` and `forks`
row-types, the `session` framing becomes actively misleading: a row in
those tables is a PR or a fork, not a session.

Concrete pain points with the current shape:

- The CLI surface bakes "session" into the command name
  (`conspectus session`), but the rows can be agents, mux sessions, or
  the mixed-union projection — and we are about to add PRs and forks.
- The `--projection {agent|mux|union}` flag implies a single
  command with three modes, which makes adding a new row-type a flag
  expansion rather than a new subcommand. Each row-type wants its own
  column registry, help text, and (eventually) row-type-specific
  flags.
- ADR 0012 records a `[session]\nprojection = "..."` config key. Once
  row-types proliferate, the analogous config grows per-row-type
  preferences (column lists, layout overrides), which fits poorly into
  a single `[session]` table.

The H-TBL-007 column registry needs a stable namespace for per-row-type
defaults. Without committing to a command shape and a config key shape,
that work would later require renaming both surfaces in lockstep with
broken backwards-compat for any user who picked up the column
customization in between.

## Decision

Replace `conspectus session [--projection X]` with a `conspectus table
<ROWS>` subcommand tree, and migrate config from `[session]` to
`[table.<rows>]` per-row-type subsections.

### CLI surface

```sh
conspectus table sessions [--wide | --width N] [--layout {columnar|card}]
                          [--scan-root PATH]... [--columns LIST]   # H-TBL-007
conspectus table mux      [...]
conspectus table union    [...]
conspectus table prs      [...]                                    # H-TBL-008
conspectus table forks    [...]                                    # H-TBL-009
```

- The row-type is a required positional. There is no default — the
  user always types `table sessions` (or another row-type) explicitly.
  Tab completion and `conspectus table --help` make the row-types
  discoverable; the previous "default projection" config knob is
  retired because the explicit positional reads better in shell
  history and avoids surprising the user with an implicit default.
- Internal `Projection` enum (ADR 0006 vocabulary) keeps its name and
  values. Each `table` subcommand maps to one variant for the
  renderer. New row-types extend the same enum.
- The existing `--wide`, `--width`, and `--layout` flags carry over to
  every row-type unchanged.
- The previous `conspectus session` subcommand is removed outright,
  with no alias. Per CLAUDE.md's no-backcompat-hacks guidance, the
  rename is a hard cut.

### Config schema

```toml
# .conspectus.toml or $XDG_CONFIG_HOME/conspectus/config.toml
[table.sessions]
# (empty in H-TBL-006; gains `columns = [...]` in H-TBL-007)

[table.mux]

[table.union]

[table.prs]    # registered when H-TBL-008 lands
[table.forks]  # registered when H-TBL-009 lands
```

- Top-level table is `[table]`. Every row-type has its own subsection
  `[table.<rows>]`.
- Unknown subsections under `[table]` are ignored with a diagnostic on
  stderr, mirroring ADR 0012's treatment of unknown keys at the
  top level.
- The previous `[session].projection` key is removed. Any value left
  in user config triggers a `ConfigDiagnostic` ("unknown section
  `session`") but does not abort the run; this gives users a one-line
  hint that the schema changed, without breaking anyone's workflow
  by aborting.

### What stays the same

- ADR 0012's precedence rules (project wins over user wins over
  defaults), file locations, and silent-falls-back-to-defaults posture
  are unchanged.
- The text-table renderer (`src/output/table.rs`), short row id
  (`node_short_id`), and width-aware truncation continue to back every
  row-type. H-TBL-006 changes only the CLI shape and the config key
  layout; no renderer behavior changes.
- The `node show` resolver (H-TBL-005) is unaffected.

## Consequences

- Adding a row-type is now a structural change (a new subcommand)
  rather than a flag-enum expansion. Each row-type owns its own
  `--help`, column registry (H-TBL-007), and any row-type-specific
  flags that emerge later.
- Users with `[session]` in `.conspectus.toml` see a stderr diagnostic
  on the next run pointing at the renamed schema. Their command-line
  flags still work because clap rejects the removed `session`
  subcommand and prints the new tree.
- ADR 0012's documented example schema (`[session]\nprojection`) is
  superseded by this ADR. The core decisions in ADR 0012 (file
  locations, precedence, silent defaults) remain authoritative.
- `docs/operations.md` is updated to document `conspectus table`,
  `[table.<rows>]`, and the absence of a default-projection knob.

## Alternatives Considered

- **Keep `conspectus session` and grow `--projection` into a row-type
  flag.** Rejected because the command name encodes a row-type that no
  longer fits the data; users typing `conspectus session --projection
  prs` to ask "show me PRs" would be a poor UX, and adding `prs` and
  `forks` to a flag enum buries them under a misleading verb.
- **`conspectus log [--rows ...]` to mirror `git log`.** Rejected
  because the data is a current-state snapshot, not a stream or a
  history. `git log`'s mental model would mislead in the opposite
  direction.
- **`conspectus table --rows {sessions|mux|prs|forks|union}` (single
  command, flag-driven).** Rejected because each row-type's column
  registry and (eventually) row-type-specific flags share one `--help`
  page under this shape, which gets noisy quickly. Subcommands give
  each row-type a clean help surface.
- **Keep a default-projection knob in config.** Rejected. With the
  positional required, the command is self-describing and resists
  the "wait, what did `conspectus session` show me again?" failure
  mode the previous default invited. If a real need for a default
  surfaces later, it can be added without further schema churn (e.g.
  `[table].default_rows = "sessions"`).
- **Alias `conspectus session` to `conspectus table sessions` for one
  release.** Rejected per CLAUDE.md's avoid-backwards-compat-hacks
  guidance. A clean cut keeps the CLI predictable and avoids carrying
  dead code.

## Open Questions Answered

- Row-type is required, not optional. There is no implicit default.
- The internal `Projection` enum is not renamed in this ADR; it
  continues to back the renderer. A future ADR may rename it to
  `RowsKind` if the renderer's public API grows beyond table
  rendering, but that is out of scope for H-TBL-006.
- `[table.<rows>]` subsections are pre-created for every registered
  row-type at the schema level; subsections for row-types Conspectus
  does not know about surface a diagnostic and are ignored.
