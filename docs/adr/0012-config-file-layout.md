# ADR 0012: Conspectus Configuration File Layout

## Status

Accepted

## Context

Phase 04 introduces the first user-facing CLI option that needs a
non-volatile default: `conspectus session --projection
{agent|mux|union}`. The default projection is a per-user / per-project
preference, not a CLI argument the user wants to retype each
invocation.

Conspectus has so far been entirely flag-driven, but the design has
always anticipated configuration:

- `docs/design.md` describes user overrides and declared link intent.
- `CLAUDE.md` requires that "user-authored link intent" live near the
  relevant repo or workspace when the relationship is project-rooted,
  and that rebuildable caches stay outside project trees.
- Future phases (declared links in Phase 5, Atelier delegation in
  Phase 6) will keep extending the same config surface.

The project needs a config file layout that:

- supports per-project overrides without making them mandatory
- supports a user-wide default that travels across projects
- follows the XDG Base Directory Specification when possible
- uses a format Conspectus already parses (TOML)
- has clear, predictable precedence rules
- is forward-compatible with future sections without breaking older
  files

## Decision

Conspectus loads configuration from two files, in this precedence
order (highest wins):

1. **Project-local**: `<repo-or-workspace>/.conspectus.toml`, walked
   upward from the current working directory until either the file is
   found or a filesystem boundary is hit (filesystem root or the user's
   `$HOME`, whichever comes first).
2. **User-level**: `$XDG_CONFIG_HOME/conspectus/config.toml`, falling
   back to `$HOME/.config/conspectus/config.toml` when
   `$XDG_CONFIG_HOME` is not set, then to
   `%APPDATA%\conspectus\config.toml` on Windows.

Both files are TOML. Missing files are not errors and load as empty
configs. Malformed files surface a diagnostic but do not crash the
run; Conspectus continues with defaults.

The initial schema is intentionally minimal:

```toml
[session]
projection = "agent"  # one of "agent" | "mux" | "union"; default "agent"
```

Unknown sections and unknown keys are ignored with a diagnostic. This
keeps older binaries forward-compatible with config files written for
newer Conspectus versions. Unknown values in a known key (for example
`projection = "ledger"`) are an error during loading rather than a
silent fallback, because they almost always indicate a typo the user
should fix.

Precedence within a single key is "project wins over user wins over
built-in default". Precedence between sections (when future sections
appear) follows the same rule independently per key.

Conspectus does not maintain a *machine-generated* cache file in the
config locations. Rebuildable caches, per CLAUDE.md, live outside the
project tree under `$XDG_CACHE_HOME/conspectus/` (or the platform
equivalent). The cache layout will be specified separately when caches
are actually introduced.

## Consequences

- The CLI gains a stable, predictable place to read user preferences
  without forcing every invocation to carry them as flags.
- Projects that need a different default (for example, a multi-mux
  workspace that prefers `mux` projection) can ship a
  `.conspectus.toml` in the project root.
- Conspectus does not write to either config file; both stay
  user-authored. The CLI may print a recommended snippet but never
  edits the file in-place without an explicit subcommand.
- The TOML schema is small enough that a future ADR can extend it
  (declared links, session filters, table column preferences) without
  reopening the precedence model.
- The upward walk for `.conspectus.toml` mirrors how tools like
  `rustfmt` and `editorconfig` discover project config, which keeps
  user mental model intact.
- The `$HOME` boundary on the upward walk avoids accidentally reading
  someone else's `.conspectus.toml` if `cwd` is outside the user's
  files (for example a shared `/tmp` checkout). This is a
  defense-in-depth choice, not a security boundary.

## Alternatives Considered

- **CLI flags only**, no config file. Rejected because the default
  projection is the kind of stable preference that belongs in user
  config, not in shell aliases.
- **Single user-level file, no project override**. Rejected because
  Conspectus is explicitly designed around per-workspace and per-fork
  user-authored intent (`docs/design.md`), so the config layout has to
  leave room for project-local overrides from day one.
- **JSON or YAML config**. Rejected. The crate already parses TOML
  (`toml`, `toml_edit` are project dependencies), and TOML is the
  Rust-ecosystem convention. Introducing YAML would require a new
  dependency without an offsetting benefit.
- **Environment variables only** (`CONSPECTUS_SESSION_PROJECTION=...`).
  Useful as an override later, but rejected as the primary surface
  because env vars are awkward to discover and document, and they
  don't extend cleanly to declared link intent in Phase 5.
- **Reuse `.editorconfig`-style cascading**. Rejected because that
  format does not natively model nested tables and would force
  Conspectus to invent its own key-flattening convention.

## Open Questions Answered

- The project-local config file lives at `.conspectus.toml` (root-
  level, hidden) and is discovered by walking upward from the current
  directory.
- The user-level config follows the XDG Base Directory Specification
  with explicit Windows fallback.
- Both files are TOML.
- Project config takes precedence over user config, which takes
  precedence over built-in defaults.
- Missing files are not errors.
- Malformed files surface a diagnostic and Conspectus continues with
  defaults; an invalid value for a known key is an error.
- Conspectus does not write to either file as part of normal CLI
  operation.
