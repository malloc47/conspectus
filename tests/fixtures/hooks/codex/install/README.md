# Codex hook install cases (synthetic)

Every file here is **synthetic**: a `config.toml` an operator might have
before `conspectus hook init codex`, written by hand in the forms TOML
allows and the Codex docs show, not copied from a real configuration.
The installer tests (`src/cli/hook_codex_tests.rs`, and the command-line
tests in `tests/cli_smoke.rs`) install into each and remove again, and
check that the operator's text comes back unchanged.

- `operator-array-of-tables.toml`: `[[hooks.SessionStart]]` and
  `[[hooks.Stop]]` groups with `[[...hooks]]` handlers, comments, and
  other tables around them.
- `inline-hooks.toml`: the whole table as `hooks = { ... }`.
- `inline-lists.toml`: inline lists under `[hooks]`, holding the entry an
  older Conspectus wrote (no timeout, `async = false`) after the
  operator's.
- `dotted-keys.toml`: `hooks.SessionStart = [...]` at the top level.
- `hooks-state.toml`: a `hooks.state` table where Codex records the hooks
  the operator trusted; the key and hash are placeholders.
- `conspectus-between-operator-groups.toml`: a Conspectus group between
  two of the operator's, for removal.
- `hooks.json`: a hook file Codex reads beside `config.toml`, which
  Conspectus never writes.
